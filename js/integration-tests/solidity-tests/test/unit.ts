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

describe("Unit tests", () => {
  const alchemyUrl = process.env.ALCHEMY_URL;
  const rpcCachePath = "./edr-cache";
  let artifacts: Artifact[], testSuiteIds: ArtifactId[];

  before(async () => {
    const results = await buildSolidityTestsInput(hre.artifacts);
    artifacts = results.artifacts;
    testSuiteIds = results.testSuiteIds;
  });

  function matchingTest(contractName: string): ArtifactId[] {
    return matchingTests(new Set([contractName]));
  }

  function matchingTests(testContractNames: Set<string>): ArtifactId[] {
    return testSuiteIds.filter((testSuiteId) => {
      return testContractNames.has(testSuiteId.name);
    });
  }

  it("SuccessAndFailure", async function () {
    const { totalTests, failedTests } = await runTestsWithStats(
      artifacts,
      matchingTest("SuccessAndFailureTest"),
      {
        projectRoot: hre.config.paths.root,
      }
    );

    assert.equal(failedTests, 1);
    assert.equal(totalTests, 2);
  });

  it("ContractEnvironment", async function () {
    const { totalTests, failedTests } = await runTestsWithStats(
      artifacts,
      matchingTest("ContractEnvironmentTest"),
      {
        projectRoot: hre.config.paths.root,
        sender: Buffer.from("976EA74026E726554dB657fA54763abd0C3a0aa9", "hex"),
        chainId: 12n,
        blockNumber: 23n,
        blockTimestamp: 45n,
      }
    );

    assert.equal(failedTests, 0);
    assert.equal(totalTests, 1);
  });

  it("GlobalFork", async function () {
    if (alchemyUrl === undefined) {
      this.skip();
    }

    const { totalTests, failedTests } = await runTestsWithStats(
      artifacts,
      matchingTest("GlobalForkTest"),
      {
        projectRoot: hre.config.paths.root,
        rpcCachePath,
        ethRpcUrl: alchemyUrl,
        forkBlockNumber: 20_000_000n,
      }
    );

    assert.equal(failedTests, 0);
    assert.equal(totalTests, 1);
  });

  it("ForkCheatcode", async function () {
    if (alchemyUrl === undefined) {
      this.skip();
    }

    const { totalTests, failedTests } = await runTestsWithStats(
      artifacts,
      matchingTest("ForkCheatcodeTest"),
      {
        projectRoot: hre.config.paths.root,
        rpcCachePath,
        rpcEndpoints: {
          alchemyMainnet: alchemyUrl,
        },
      }
    );

    assert.equal(failedTests, 0);
    assert.equal(totalTests, 1);
  });
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
