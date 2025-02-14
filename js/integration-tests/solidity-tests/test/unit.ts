import { assert } from "chai";
import { assertStackTraces, TestContext } from "./testContext";
import { runAllSolidityTests } from "@nomicfoundation/edr-helpers";
import { EdrContext, ReturnData, StackTraceEntryType } from "@ignored/edr";
import { getContext } from "@ignored/edr/build-test/test/helpers";

describe("Unit tests", () => {
  let testContext: TestContext;

  before(async () => {
    testContext = await TestContext.setup();
  });

  // Empty test suite should still return a result.
  it("Empty", async function () {
    const { totalTests, failedTests } =
      await testContext.runTestsWithStats("EmptyTest");

    assert.equal(failedTests, 0);
    assert.equal(totalTests, 0);
  });

  it("SuccessAndFailure", async function () {
    const { totalTests, failedTests, stackTraces } =
      await testContext.runTestsWithStats("SuccessAndFailureTest");

    assertStackTraces(
      stackTraces.get("testThatFails()"),
      "revert: 1 is not equal to 2",
      [{ contract: "SuccessAndFailureTest", function: "testThatFails" }]
    );

    assert.equal(failedTests, 1);
    assert.equal(totalTests, 2);
  });

  it("ContractEnvironment", async function () {
    const { totalTests, failedTests } = await testContext.runTestsWithStats(
      "ContractEnvironmentTest",
      {
        sender: Buffer.from("976EA74026E726554dB657fA54763abd0C3a0aa9", "hex"),
        chainId: 12n,
        blockNumber: 23n,
        blockTimestamp: 45n,
      }
    );

    assert.equal(failedTests, 0);
    assert.equal(totalTests, 1);
  });

  describe("IsolateMode", function () {
    it("IsolateMode on", async function () {
      const { totalTests, failedTests } = await testContext.runTestsWithStats(
        "IsolateTest",
        {
          isolate: true,
        }
      );

      assert.equal(failedTests, 0);
      assert.equal(totalTests, 1);
    });

    it("IsolateMode off", async function () {
      const { totalTests, failedTests } =
        await testContext.runTestsWithStats("IsolateTest");

      assert.equal(failedTests, 1);
      assert.equal(totalTests, 1);
    });
  });

  describe("TestFail", function () {
    it("TestFail on", async function () {
      const { totalTests, failedTests } = await testContext.runTestsWithStats(
        "TestFailTest",
        {
          testFail: true,
        }
      );

      // Reverting test starting with `testFail` should be reported as success if `testFail` is on
      assert.equal(failedTests, 0);
      assert.equal(totalTests, 1);
    });

    it("TestFail off", async function () {
      const { totalTests, failedTests } =
        await testContext.runTestsWithStats("TestFailTest");

      // Reverting test starting with `testFail` should be reported as failure if `testFail` is off
      assert.equal(failedTests, 1);
      assert.equal(totalTests, 1);
    });
  });

  it("EnvVarTest", async function () {
    process.env._EDR_SOLIDITY_TESTS_GET_ENV_TEST_KEY =
      "_edrSolidityTestsGetEnvTestVal";

    const { totalTests, failedTests } =
      await testContext.runTestsWithStats("EnvVarTest");

    assert.equal(failedTests, 0);
    assert.equal(totalTests, 1);
  });

  it("GlobalFork", async function () {
    if (testContext.rpcUrl === undefined) {
      this.skip();
    }

    const { totalTests, failedTests } = await testContext.runTestsWithStats(
      "GlobalForkTest",
      {
        ethRpcUrl: testContext.rpcUrl,
        forkBlockNumber: 20_000_000n,
      }
    );

    assert.equal(failedTests, 0);
    assert.equal(totalTests, 1);
  });

  it("ForkCheatcode", async function () {
    if (testContext.rpcUrl === undefined) {
      this.skip();
    }

    const { totalTests, failedTests } = await testContext.runTestsWithStats(
      "ForkCheatcodeTest",
      {
        rpcEndpoints: {
          alchemyMainnet: testContext.rpcUrl,
        },
      }
    );

    assert.equal(failedTests, 0);
    assert.equal(totalTests, 1);
  });

  it("ForkCheatcodeStacktraceTest", async function () {
    if (testContext.rpcUrl === undefined) {
      this.skip();
    }

    const testName = "ForkCheatcodeStacktraceTest";
    const config = {
      rpcEndpoints: {
        alchemyMainnet: testContext.rpcUrl,
      },
    };

    let testContracts = testContext.matchingTest(testName);

    const suiteResults = await runAllSolidityTests(
      testContext.artifacts,
      testContracts,
      testContext.tracingConfig,
      {
        ...testContext.defaultConfig(),
        ...config,
      }
    );

    const blockNumberDuringExecution =
      suiteResults[0].testResults[0].decodedLogs[0];
    // Sanity check
    assert(
      Number(blockNumberDuringExecution) > 2000000,
      `Invalid block number during execution: ${blockNumberDuringExecution}`
    );

    const stackTraces = suiteResults[0].testResults[0].stackTrace();
    if (stackTraces == null || stackTraces.length == 0) {
      throw new Error("No stack traces");
    }

    const stackTraceEntry = stackTraces[0];

    switch (stackTraceEntry.type) {
      case StackTraceEntryType.REVERT_ERROR: {
        const returnData = new ReturnData(stackTraceEntry.returnData);
        // The revert message has the block number during re-execution
        assert.equal(blockNumberDuringExecution, returnData.decodeError());
        break;
      }
      default: {
        throw new Error(`Unexpected stack trace entry: ${stackTraceEntry}`);
      }
    }
  }).timeout(120000);

  it("FailingSetup", async function () {
    const { totalTests, failedTests, stackTraces } =
      await testContext.runTestsWithStats("FailingSetupTest");

    assertStackTraces(
      stackTraces.get("setUp()"),
      "invalid rpc url: nonExistentForkAlias",
      [{ contract: "FailingSetupTest", function: "setUp" }]
    );

    assert.equal(failedTests, 1);
    assert.equal(totalTests, 1);
  });

  it("FailingDeploy", async function () {
    const { totalTests, failedTests, stackTraces } =
      await testContext.runTestsWithStats("FailingDeployTest");

    assertStackTraces(stackTraces.get("setUp()"), "revert: Deployment failed", [
      { contract: "FailingDeployTest", function: "constructor" },
    ]);

    assert.equal(failedTests, 1);
    assert.equal(totalTests, 1);
  });
});
