import type { HardhatRuntimeEnvironment } from "hardhat/types/hre";
import * as path from "node:path";
import { fileURLToPath } from "node:url";
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

export type * from "hardhat/hre";

import { resolveFromRoot } from "@nomicfoundation/hardhat-utils/path";
import {
  buildEdrArtifactsWithMetadata,
  getBuildInfosAndOutputs,
  EdrArtifactWithMetadata,
} from "hardhat/internal/builtin-plugins/solidity-test/edr-artifacts";
import {
  isTestSuiteArtifact,
  warnDeprecatedTestFail,
} from "hardhat/internal/builtin-plugins/solidity-test/helpers";
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

  // EDR needs all artifacts (contracts + tests). In unified mode (the default
  // since Hardhat 3.4.x), both scopes point to the same directory, so iterating
  // both would load every artifact twice.
  const scopes = hre.config.solidity.splitTestsCompilation
    ? (["contracts", "tests"] as const)
    : (["contracts"] as const);
  const edrArtifacts: EdrArtifactWithMetadata[] = [];
  const buildInfos: BuildInfoAndOutput[] = [];
  for (const scope of scopes) {
    const artifactsDir = await hre.solidity.getArtifactsDirectory(scope);
    const artifactManager = new ArtifactManagerImplementation(artifactsDir);
    edrArtifacts.push(
      ...(await buildEdrArtifactsWithMetadata(artifactManager))
    );
    buildInfos.push(...(await getBuildInfosAndOutputs(artifactManager)));
  }

  // Duplicate artifacts break `runAllSolidityTests`: the native runner dedupes
  // suites by ArtifactId, so it fires fewer callbacks than `testSuites.length`
  // and the promise never resolves. Warn loudly if Hardhat ever feeds us
  // duplicates so the failure is visible instead of a silent hang.
  const seenIds = new Set<string>();
  const duplicateIds: string[] = [];
  for (const { edrArtifact } of edrArtifacts) {
    const key = `${edrArtifact.id.source}:${edrArtifact.id.name}@${edrArtifact.id.solcVersion}`;
    if (seenIds.has(key)) {
      duplicateIds.push(key);
    } else {
      seenIds.add(key);
    }
  }
  if (duplicateIds.length > 0) {
    console.warn(
      `Warning: buildSolidityTestsInput loaded ${duplicateIds.length} duplicate artifact(s); ` +
        `runAllSolidityTests will hang because the native runner dedupes suites. ` +
        `Duplicates: ${duplicateIds.join(", ")}`
    );
  }

  const sourceNameToUserSourceName = new Map(
    edrArtifacts.map(({ userSourceName, edrArtifact }) => [
      edrArtifact.id.source,
      userSourceName,
    ])
  );

  edrArtifacts.forEach(({ userSourceName, edrArtifact }) => {
    if (
      testRootPaths.includes(
        resolveFromRoot(hre.config.paths.root, userSourceName)
      ) &&
      isTestSuiteArtifact(edrArtifact)
    ) {
      warnDeprecatedTestFail(edrArtifact, sourceNameToUserSourceName);
    }
  });

  const testSuiteIds = edrArtifacts
    .filter(({ userSourceName }) =>
      testRootPaths.includes(
        resolveFromRoot(hre.config.paths.root, userSourceName)
      )
    )
    .filter(({ edrArtifact }) => isTestSuiteArtifact(edrArtifact))
    .map(({ edrArtifact }) => edrArtifact.id);

  const artifacts = edrArtifacts.map(({ edrArtifact }) => edrArtifact);

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

function buildMutex() {
  if (BUILD_MUTEX === undefined) {
    BUILD_MUTEX = new MultiProcessMutex("edr-helpers-build-mutex");
  }
  return BUILD_MUTEX;
}
