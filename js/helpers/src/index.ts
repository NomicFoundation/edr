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
  BuildInfoAndOutput,
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
import { warnDeprecatedTestFail } from "hardhat/internal/builtin-plugins/solidity-test/helpers";
import { ArtifactManagerImplementation } from "hardhat/internal/builtin-plugins/artifacts/artifact-manager";

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
    let solidityTestResult: SolidityTestResult | undefined;
    let isTestComplete = false;

    const tryResolve = () => {
      if (isTestComplete && resultsFromCallback.length === testSuites.length) {
        resolve([solidityTestResult!, resultsFromCallback]);
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
        solidityTestResult = result;
        isTestComplete = true;
        tryResolve();
      })
      .catch(reject);
  });
}

/*
 Build Solidity tests in a Hardhat v3 project.
 Based on https://github.com/NomicFoundation/hardhat/blob/6e9903439fcf145e4b603be60c4c1c036095a241/v-next/hardhat/src/internal/builtin-plugins/solidity-test/task-action.ts
 */
export async function buildSolidityTestsInput(
  hre: HardhatRuntimeEnvironment
): Promise<{
  artifacts: Artifact[];
  testSuiteIds: ArtifactId[];
  tracingConfig: TracingConfigWithBuffers;
}> {
  let testRootPaths: string[];

  // Cache assumes one build process at a time.
  await buildMutex().use(async () => {
    await hre.tasks.getTask("build").run({
      noTests: true,
      quiet: true,
    });
    // Run the build task for test files
    const result: { testRootPaths: string[] } = await hre.tasks
      .getTask("build")
      .run({
        // If no specific files are passed, it means compile all source files
        files: [],
        noContracts: true,
        quiet: true,
      });
    testRootPaths = result.testRootPaths;
  });

  // EDR needs all artifacts (contracts + tests)
  const edrArtifacts: Array<{
    edrAtifact: Artifact;
    userSourceName: string;
  }> = [];
  const buildInfos: BuildInfoAndOutput[] = [];
  for (const scope of ["contracts", "tests"] as const) {
    const artifactsDir = await hre.solidity.getArtifactsDirectory(scope);
    const artifactManager = new ArtifactManagerImplementation(artifactsDir);
    edrArtifacts.push(...(await getEdrArtifacts(artifactManager)));
    buildInfos.push(...(await getBuildInfos(artifactManager)));
  }

  const sourceNameToUserSourceName = new Map(
    edrArtifacts.map(({ userSourceName, edrAtifact }) => [
      edrAtifact.id.source,
      userSourceName,
    ])
  );

  edrArtifacts.forEach(({ userSourceName, edrAtifact }) => {
    if (
      testRootPaths.includes(
        resolveFromRoot(hre.config.paths.root, userSourceName)
      ) &&
      isTestSuiteArtifact(edrAtifact)
    ) {
      warnDeprecatedTestFail(edrAtifact, sourceNameToUserSourceName);
    }
  });

  const testSuiteIds = edrArtifacts
    .filter(({ userSourceName }) =>
      testRootPaths.includes(
        resolveFromRoot(hre.config.paths.root, userSourceName)
      )
    )
    .filter(({ edrAtifact }) => isTestSuiteArtifact(edrAtifact))
    .map(({ edrAtifact }) => edrAtifact.id);

  const artifacts = edrArtifacts.map(({ edrAtifact }) => edrAtifact);

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
