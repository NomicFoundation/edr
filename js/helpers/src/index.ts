import * as path from "node:path";
import { fileURLToPath } from "node:url";
import { getAllFilesMatching } from "@nomicfoundation/hardhat-utils/fs";
import { MultiProcessMutex } from "@nomicfoundation/hardhat-utils/synchronization";
import {
  Artifact,
  ArtifactId,
  EdrContext,
  SuiteResult,
  SolidityTestRunnerConfigArgs,
  TestResult,
  TracingConfigWithBuffers,
} from "@nomicfoundation/edr";
import { HardhatRuntimeEnvironment } from "hardhat/types/hre";
import { BuildOptions } from "hardhat/types/solidity";
import { Abi } from "hardhat/types/artifacts";

import {
  getArtifacts,
  getBuildInfos,
  throwIfSolidityBuildFailed,
} from "./build-results.js";

let BUILD_MUTEX: MultiProcessMutex | undefined;

/**
 * Run all the given solidity tests and returns the whole results after finishing.
 */
export async function runAllSolidityTests(
  context: EdrContext,
  chainType: string,
  artifacts: Artifact[],
  testSuites: ArtifactId[],
  tracingConfig: TracingConfigWithBuffers,
  configArgs: SolidityTestRunnerConfigArgs,
  testResultCallback: (
    suiteResult: SuiteResult,
    testResult: TestResult
  ) => void = () => {}
): Promise<SuiteResult[]> {
  return new Promise((resolve, reject) => {
    const resultsFromCallback: SuiteResult[] = [];

    context
      .runSolidityTests(
        chainType,
        artifacts,
        testSuites,
        configArgs,
        tracingConfig,
        (suiteResult: SuiteResult) => {
          for (const testResult of suiteResult.testResults) {
            testResultCallback(suiteResult, testResult);
          }

          resultsFromCallback.push(suiteResult);
          if (resultsFromCallback.length === testSuites.length) {
            resolve(resultsFromCallback);
          }
        }
      )
      .catch(reject);
  });
}

/*
 Build Solidity tests in a Hardhat v3 project.
 Based on https://github.com/NomicFoundation/hardhat/blob/6acdfa0a7e332e26a3f0fbda61edb4a4971a542e/v-next/hardhat/src/internal/builtin-plugins/solidity-test/task-action.ts
 */
export async function buildSolidityTestsInput(
  hre: HardhatRuntimeEnvironment
): Promise<{
  artifacts: Artifact[];
  testSuiteIds: ArtifactId[];
  tracingConfig: TracingConfigWithBuffers;
}> {
  let rootFilePaths = (
    await Promise.all([
      getAllFilesMatching(hre.config.paths.tests.solidity, (f) =>
        f.endsWith(".sol")
      ),
      ...hre.config.paths.sources.solidity.map(async (dir) => {
        // This is changed from Hardhat: it currently filters for ".t.sol" which is probably a mistake.
        return getAllFilesMatching(dir, (f) => f.endsWith(".sol"));
      }),
    ])
  ).flat(1);
  // NOTE: We remove duplicates in case there is an intersection between
  // the tests.solidity paths and the sources paths
  rootFilePaths = Array.from(new Set(rootFilePaths));
  const buildOptions: BuildOptions = {
    force: false,
    buildProfile: hre.globalOptions.buildProfile,
    quiet: true,
  };

  // Cache assumes one build process at a time.
  const results = await buildMutex().use(() =>
    hre.solidity.build(rootFilePaths, buildOptions)
  );

  throwIfSolidityBuildFailed(results);

  const buildInfos = await getBuildInfos(results, hre.artifacts);
  const artifacts = await getArtifacts(results);
  const testSuiteIds = artifacts
    .filter(isTestSuiteArtifact)
    .map((artifact) => artifact.id);

  const tracingConfig: TracingConfigWithBuffers = {
    buildInfos,
    ignoreContracts: false,
  };

  return { artifacts, testSuiteIds, tracingConfig };
}

/* Get the directory name of the current file based on the `import.meta.url`*/
export function dirName(importUrl: string) {
  const fileName = fileURLToPath(importUrl);
  return path.dirname(fileName);
}

// Copied from <https://github.com/NomicFoundation/hardhat/blob/58463c16270ae154b6671d2d2eea2ba95d024d2e/v-next/hardhat/src/internal/builtin-plugins/solidity-test/helpers.ts>
function isTestSuiteArtifact(artifact: Artifact): boolean {
  const abi: Abi = JSON.parse(artifact.contract.abi);
  return abi.some(({ type, name }) => {
    if (type === "function" && typeof name === "string") {
      return name.startsWith("test") || name.startsWith("invariant");
    }
    return false;
  });
}

function buildMutex() {
  if (BUILD_MUTEX === undefined) {
    BUILD_MUTEX = new MultiProcessMutex("edr-helpers-build-mutex");
  }
  return BUILD_MUTEX;
}
