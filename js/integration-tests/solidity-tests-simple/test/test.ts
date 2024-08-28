import type {
  Artifact,
  ArtifactId,
  SolidityTestRunnerConfigArgs,
} from "@nomicfoundation/edr";
import { runAllSolidityTests } from "@nomicfoundation/edr-helpers";
import type { Artifacts } from "hardhat/types";
import { assert } from "chai";
import hre from "hardhat";

it("test", async function () {
  const { artifacts, testSuiteIds } = await buildSolidityTestsInput(
    hre.artifacts
  );

  const { totalTests, failedTests } = await runTestsWithStats(
    artifacts,
    testSuiteIds,
    {
      projectRoot: hre.config.paths.root,
    }
  );

  assert.equal(failedTests, 1);
  assert.equal(totalTests, 2);
});

interface SolidityTestsRunResult {
  totalTests: number;
  failedTests: number;
}

async function runTestsWithStats(
  artifacts: Artifact[],
  testSuiteIds: ArtifactId[],
  config: SolidityTestRunnerConfigArgs
): Promise<SolidityTestsRunResult> {
  let totalTests = 0;
  let failedTests = 0;

  const suiteResults = await runAllSolidityTests(
    artifacts,
    testSuiteIds,
    config
  );

  for (const suiteResult of suiteResults) {
    for (const testResult of suiteResult.testResults) {
      let failed = testResult.status === "Failure";
      totalTests++;
      if (failed) {
        failedTests++;
      }
    }
  }
  return { totalTests, failedTests };
}

async function buildSolidityTestsInput(
  hardhatArtifacts: Artifacts
): Promise<{ artifacts: Artifact[]; testSuiteIds: ArtifactId[] }> {
  const fqns = await hardhatArtifacts.getAllFullyQualifiedNames();
  const artifacts: Artifact[] = [];
  const testSuiteIds: ArtifactId[] = [];

  for (const fqn of fqns) {
    const artifact = hardhatArtifacts.readArtifactSync(fqn);
    const buildInfo = hardhatArtifacts.getBuildInfoSync(fqn);

    if (buildInfo === undefined) {
      throw new Error(`Build info not found for contract ${fqn}`);
    }

    const id = {
      name: artifact.contractName,
      solcVersion: buildInfo.solcVersion,
      source: artifact.sourceName,
    };

    const contract = {
      abi: JSON.stringify(artifact.abi),
      bytecode: artifact.bytecode,
      deployedBytecode: artifact.deployedBytecode,
    };

    artifacts.push({ id, contract });
    testSuiteIds.push(id);
  }

  return { artifacts, testSuiteIds };
}
