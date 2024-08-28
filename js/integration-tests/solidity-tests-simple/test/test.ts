import type {
  Artifact,
  ArtifactId,
  SolidityTestRunnerConfigArgs,
} from "@nomicfoundation/edr";
import {
  buildSolidityTestsInput,
  runAllSolidityTests,
} from "@nomicfoundation/edr-helpers";
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
