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
  SolidityTestResult,
} from "@nomicfoundation/edr";
import { HardhatRuntimeEnvironment } from "hardhat/types/hre";
import { BuildOptions } from "hardhat/types/solidity";
import { Abi } from "hardhat/types/artifacts";

import { resolveFromRoot } from "@nomicfoundation/hardhat-utils/path";
import {
  getBuildInfos,
  getEdrArtifacts,
} from "hardhat/internal/builtin-plugins/solidity-test/edr-artifacts";
import { throwIfSolidityBuildFailed } from "hardhat/internal/builtin-plugins/solidity/build-results";

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
): Promise<[SolidityTestResult, SuiteResult[]]> {
  return new Promise((resolve, reject) => {
    const resultsFromCallback: SuiteResult[] = [];
    let testResult: SolidityTestResult | undefined;
    let isTestComplete = false;

    const tryResolve = () => {
      if (isTestComplete && resultsFromCallback.length === testSuites.length) {
        resolve([testResult!, resultsFromCallback]);
      }
    };

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
          tryResolve();
        }
      )
      .then((result) => {
        testResult = result;
        isTestComplete = true;
        tryResolve();
      })
      .catch(reject);
  });
}

/*
 Build Solidity tests in a Hardhat v3 project.
 Based on https://github.com/NomicFoundation/hardhat/blob/c1e3202e7dbcf39588e687a7263d035751a074df/v-next/hardhat/src/internal/builtin-plugins/solidity-test/task-action.ts
 */
export async function buildSolidityTestsInput(
  hre: HardhatRuntimeEnvironment
): Promise<{
  artifacts: Artifact[];
  testSuiteIds: ArtifactId[];
  tracingConfig: TracingConfigWithBuffers;
}> {
  // Cache assumes one build process at a time.
  await buildMutex().use(async () => {
    // NOTE: We run the compile task first to ensure all the artifacts for them are generated
    // Then, we compile just the test sources. We don't do it in one go because the user
    // is likely to use different compilation options for the tests and the sources.
    await hre.tasks.getTask("compile").run({ quiet: true });
  });

  // NOTE: A test file is either a file with a `.sol` extension in the `tests.solidity`
  // directory or a file with a `.t.sol` extension in the `sources.solidity` directory
  let rootFilePaths = (
    await Promise.all([
      getAllFilesMatching(hre.config.paths.tests.solidity, (f) =>
        f.endsWith(".sol")
      ),
      ...hre.config.paths.sources.solidity.map(async (dir) => {
        return getAllFilesMatching(dir, (f) => f.endsWith(".t.sol"));
      }),
    ])
  ).flat(1);
  // NOTE: We remove duplicates in case there is an intersection between
  // the tests.solidity paths and the sources paths
  rootFilePaths = Array.from(new Set(rootFilePaths));
  const buildOptions: BuildOptions = {
    force: false,
    buildProfile: hre.globalOptions.buildProfile ?? "default",
    quiet: true,
  };

  // Cache assumes one build process at a time.
  const results = await buildMutex().use(() =>
    hre.solidity.build(rootFilePaths, buildOptions)
  );
  throwIfSolidityBuildFailed(results);

  const buildInfos = await getBuildInfos(hre.artifacts);
  const edrArtifacts = await getEdrArtifacts(hre.artifacts);
  const testSuiteIds = edrArtifacts
    .filter(({ userSourceName }) =>
      rootFilePaths.includes(
        resolveFromRoot(hre.config.paths.root, userSourceName)
      )
    )
    .filter(({ edrAtifact }) => isTestSuiteArtifact(edrAtifact))
    .map(({ edrAtifact }) => edrAtifact.id);

  const tracingConfig: TracingConfigWithBuffers = {
    buildInfos,
    ignoreContracts: false,
  };

  const artifacts = edrArtifacts.map(({ edrAtifact }) => edrAtifact);

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
