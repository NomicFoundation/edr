import type { Artifacts as HardhatArtifacts } from "hardhat/types";
import fsExtra from "fs-extra";

import {
  ArtifactId,
  SuiteResult,
  runSolidityTests,
  Artifact,
  SolidityTestRunnerConfigArgs,
  TestResult,
} from "@ignored/edr";

/**
 * Run all the given solidity tests and returns the whole results after finishing.
 */
export async function runAllSolidityTests(
  artifacts: Artifact[],
  testSuites: ArtifactId[],
  tracingConfig: TracingConfigWithBuffer,
  configArgs: SolidityTestRunnerConfigArgs,
  testResultCallback: (
    suiteResult: SuiteResult,
    testResult: TestResult
  ) => void = () => {}
): Promise<SuiteResult[]> {
  return new Promise((resolve, reject) => {
    const resultsFromCallback: SuiteResult[] = [];

    runSolidityTests(
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
      },
      reject
    );
  });
}

export async function buildSolidityTestsInput(
  hardhatArtifacts: HardhatArtifacts,
  isTestArtifact: (artifact: Artifact) => boolean = () => true
): Promise<{
  artifacts: Artifact[];
  testSuiteIds: ArtifactId[];
  tracingConfig: TracingConfigWithBuffer;
}> {
  const fqns = await hardhatArtifacts.getAllFullyQualifiedNames();
  const artifacts: Artifact[] = [];
  const testSuiteIds: ArtifactId[] = [];

  for (const fqn of fqns) {
    const hardhatArtifact = hardhatArtifacts.readArtifactSync(fqn);
    const buildInfo = hardhatArtifacts.getBuildInfoSync(fqn);

    if (buildInfo === undefined) {
      throw new Error(`Build info not found for contract ${fqn}`);
    }

    const id = {
      name: hardhatArtifact.contractName,
      solcVersion: buildInfo.solcVersion,
      source: hardhatArtifact.sourceName,
    };

    const contract = {
      abi: JSON.stringify(hardhatArtifact.abi),
      bytecode: hardhatArtifact.bytecode,
      linkReferences: hardhatArtifact.linkReferences,
      deployedBytecode: hardhatArtifact.deployedBytecode,
      deployedLinkReferences: hardhatArtifact.deployedLinkReferences,
    };

    const artifact = { id, contract };
    artifacts.push(artifact);
    if (isTestArtifact(artifact)) {
      testSuiteIds.push(artifact.id);
    }
  }

  const tracingConfig = await makeTracingConfig(hardhatArtifacts);

  return { artifacts, testSuiteIds, tracingConfig };
}

// This is a copy of an internal Hardhat function that loads artifacts
// https://github.com/NomicFoundation/hardhat/blob/dd19b668e3a68085eea87f96dc05e65ae52f0ce3/packages/hardhat-core/src/internal/hardhat-network/provider/provider.ts#L506
export async function makeTracingConfig(
  artifacts: HardhatArtifacts
): Promise<TracingConfigWithBuffer> {
  const buildInfos = [];

  const buildInfoFiles = await artifacts.getBuildInfoPaths();

  for (const buildInfoFile of buildInfoFiles) {
    const buildInfo = await fsExtra.readFile(buildInfoFile);
    buildInfos.push(buildInfo);
  }

  return {
    buildInfos,
    ignoreContracts: undefined,
  };
}

export interface TracingConfigWithBuffer {
  buildInfos: Uint8Array[];
  ignoreContracts: boolean | undefined;
}
