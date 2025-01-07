import type { Artifacts as HardhatArtifacts } from "hardhat/types";
import fsExtra from "fs-extra";
import semver from "semver";

import {
  ArtifactId,
  SuiteResult,
  runSolidityTests,
  Artifact,
  SolidityTestRunnerConfigArgs,
  TestResult,
} from "@ignored/edr";
import { TracingConfig } from "hardhat/internal/hardhat-network/provider/node-types";
import { FIRST_SOLC_VERSION_SUPPORTED } from "hardhat/internal/hardhat-network/stack-traces/constants";

/**
 * Run all the given solidity tests and returns the whole results after finishing.
 */
export async function runAllSolidityTests(
  artifacts: Artifact[],
  testSuites: ArtifactId[],
  tracingConfig: TracingConfig,
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
  tracingConfig: TracingConfig;
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
      deployedBytecode: hardhatArtifact.deployedBytecode,
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

// TODO: This is a temporary workaround for creating the tracing config.
// Based on https://github.com/NomicFoundation/hardhat/blob/93bc3801849d0a761b659c472ada29983ae380c5/packages/hardhat-core/src/internal/hardhat-network/provider/provider.ts
async function makeTracingConfig(
  artifacts: HardhatArtifacts
): Promise<TracingConfig> {
  const buildInfos = [];

  const buildInfoFiles = await artifacts.getBuildInfoPaths();

  for (const buildInfoFile of buildInfoFiles) {
    const buildInfo = await fsExtra.readJson(buildInfoFile);
    if (semver.gte(buildInfo.solcVersion, FIRST_SOLC_VERSION_SUPPORTED)) {
      buildInfos.push(buildInfo);
    }
  }

  return {
    buildInfos,
  };
}
