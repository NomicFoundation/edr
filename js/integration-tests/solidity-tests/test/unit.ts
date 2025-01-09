import { assert } from "chai";
import { TestContext } from "./testContext";
import { StackTraceEntryType } from "@ignored/edr";

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

    const results = stackTraces.get("testThatFails()");
    if (results === undefined) {
      console.log(stackTraces);
      throw new Error("testThatFails not found in stackTraces");
    }

    const lastStackTraceEntry = results[0];
    if (lastStackTraceEntry === undefined) {
      throw new Error("lastStackTraceEntry not found");
    }
    if (lastStackTraceEntry.sourceReference === undefined) {
      throw new Error("sourceReference not found");
    }
    assert.equal(lastStackTraceEntry.type, StackTraceEntryType.REVERT_ERROR);
    assert.equal(
      lastStackTraceEntry.sourceReference.contract,
      "SuccessAndFailureTest"
    );
    assert.equal(lastStackTraceEntry.sourceReference.function, "testThatFails");
    assert.equal(lastStackTraceEntry.sourceReference.line, 11);
    assert(
      lastStackTraceEntry.sourceReference.sourceContent.includes(
        "function testThatFails()"
      )
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
    assert.equal(totalTests, 2);
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

  it("FailingSetup", async function () {
    const { totalTests, failedTests, stackTraces } =
      await testContext.runTestsWithStats("FailingSetupTest");

    assert.equal(failedTests, 1);
    assert.equal(totalTests, 1);

    const results = stackTraces.get("setUp()");
    if (results === undefined) {
      console.log(stackTraces);
      throw new Error("setUp not found in stackTraces");
    }

    const callEntry = results[0];
    console.log(callEntry);
    if (callEntry === undefined) {
      throw new Error("call entry not found");
    }
    if (callEntry.sourceReference === undefined) {
      throw new Error("sourceReference not found");
    }
    assert.equal(callEntry.type, StackTraceEntryType.CALLSTACK_ENTRY);
    assert.equal(callEntry.sourceReference.contract, "FailingSetupTest");
    assert.equal(callEntry.sourceReference.function, "setUp");
    assert.equal(callEntry.sourceReference.line, 11);
    assert(
      callEntry.sourceReference.sourceContent.includes("function setUp()")
    );

    const revertEntry = results[1];
    if (revertEntry === undefined) {
      throw new Error("revert entry not found");
    }
    if (revertEntry.sourceReference === undefined) {
      throw new Error("sourceReference not found");
    }
    // TODO figure out what error type this should be. Should the source map to the interface file?
    throw new Error("todo");
  });
});
