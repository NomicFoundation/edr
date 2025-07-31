import assert from "node:assert/strict";
import { before, describe, it } from "node:test";
import {
  assertImpureCheatcode,
  assertStackTraces,
  TestContext,
} from "./testContext.js";
import {
  L1_CHAIN_TYPE,
  OP_CHAIN_TYPE,
  SuiteResult,
  TestStatus,
} from "@nomicfoundation/edr";

describe("Unit tests", () => {
  let testContext: TestContext;

  before(async () => {
    testContext = await TestContext.setup();
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

  it("Latest global fork stack trace", async function (t) {
    if (testContext.rpcUrl === undefined) {
      return t.skip();
    }

    const { totalTests, failedTests, stackTraces } =
      await testContext.runTestsWithStats("SuccessAndFailureTest", {
        ethRpcUrl: testContext.rpcUrl,
      });

    assert.equal(failedTests, 1);
    assert.equal(totalTests, 2);
    // When using forking from latest block, no stack trace should be generated as re-execution is unsafe.
    const stackTrace = stackTraces.get("testThatFails()");
    if (
      stackTrace === undefined ||
      stackTrace.stackTrace?.kind !== "UnsafeToReplay"
    ) {
      throw new Error(
        `Expected unsafe to replay stack trace, instead it is: {stackTrace}`
      );
    }
    assert.equal(stackTrace.stackTrace.globalForkLatest, true);
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

  it("GlobalFork", async function (t) {
    if (testContext.rpcUrl === undefined) {
      return t.skip();
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

  it("ForkCheatcode", async function (t) {
    if (testContext.rpcUrl === undefined) {
      return t.skip();
    }

    const { totalTests, failedTests } = await testContext.runTestsWithStats(
      "ForkCheatcodeTest",
      {
        rpcEndpoints: {
          alchemyMainnet: testContext.rpcUrl!,
        },
      }
    );

    assert.equal(failedTests, 0);
    assert.equal(totalTests, 1);
  });

  it("Latest fork cheatcode", async function (t) {
    if (testContext.rpcUrl === undefined) {
      return t.skip();
    }

    const { totalTests, failedTests, stackTraces } =
      await testContext.runTestsWithStats("LatestForkCheatcodeTest", {
        rpcEndpoints: {
          alchemyMainnet: testContext.rpcUrl!,
        },
      });

    assert.equal(failedTests, 1);
    assert.equal(totalTests, 1);

    let stackTrace = stackTraces.get("testThatFails()");
    assertImpureCheatcode(stackTrace, "createSelectFork");
  });

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

  it("LinkingTest", async function () {
    const { totalTests, failedTests } =
      await testContext.runTestsWithStats("LinkingTest");

    assert.equal(failedTests, 0);
    assert.equal(totalTests, 1);
  });

  it("LinkingTest", async function () {
    const { totalTests, failedTests } =
      await testContext.runTestsWithStats("LinkingTest");

    assert.equal(failedTests, 0);
    assert.equal(totalTests, 1);
  });

  it("CounterDifferentSolc", async function () {
    const { totalTests, failedTests } = await testContext.runTestsWithStats(
      "CounterDifferentSolcTest"
    );

    assert.equal(failedTests, 0);
    assert.equal(totalTests, 1);
  });

  it("CounterSameSolc", async function () {
    const { totalTests, failedTests } = await testContext.runTestsWithStats(
      "CounterSameSolcTest"
    );

    assert.equal(failedTests, 0);
    assert.equal(totalTests, 1);
  });

  it("UnsupportedCheatcode", async function () {
    const { totalTests, failedTests, stackTraces } =
      await testContext.runTestsWithStats("UnsupportedCheatcodeTest");

    assertStackTraces(
      stackTraces.get("testUnsupportedCheatcode()"),
      "cheatcode 'broadcast()' is not supported",
      [
        {
          contract: "UnsupportedCheatcodeTest",
          function: "testUnsupportedCheatcode",
          message: "cheatcode 'broadcast()' is not supported",
          line: 9,
        },
      ]
    );
    assert.equal(failedTests, 1);
    assert.equal(totalTests, 1);
  });

  it("L1Chain", async function () {
    const { totalTests, failedTests, stackTraces } =
      await testContext.runTestsWithStats(
        "L1ChainTest",
        undefined,
        L1_CHAIN_TYPE
      );

    assert.equal(totalTests, 1);
    assert.equal(failedTests, 0);
  });

  it("OpChain", async function () {
    const { totalTests, failedTests } = await testContext.runTestsWithStats(
      "OpChainTest",
      undefined,
      OP_CHAIN_TYPE
    );

    assert.equal(totalTests, 1);
    assert.equal(failedTests, 0);
  });

  it("Gas snapshot cheatcodes", async function () {
    const { totalTests, failedTests, suiteResults } =
      await testContext.runTestsWithStats("GasSnapshotTest", {}, L1_CHAIN_TYPE);

    assert.equal(totalTests, 12);
    assert.equal(failedTests, 0);

    let snapshots = new Map<string, Map<string, string>>();

    for (const suiteResult of suiteResults) {
      for (const testResult of suiteResult.testResults) {
        assert.notEqual(testResult.valueSnapshotGroups, undefined);

        const snapshotGroups = testResult.valueSnapshotGroups!;

        assert(
          snapshotGroups.length > 0,
          "All gas snapshot tests should have at least one scoped snapshot"
        );

        // Collect all snapshots from the groups
        for (const group of snapshotGroups) {
          let snapshot = snapshots.get(group.name);
          if (snapshot === undefined) {
            snapshot = new Map<string, string>();
            snapshots.set(group.name, snapshot);
          }

          for (const entry of group.entries) {
            snapshot.set(entry.name, entry.value);
          }
        }
      }
    }

    assert.deepEqual(
      snapshots,
      new Map([
        [
          "CustomGroup",
          new Map([
            ["e", "456"],
            ["i", "456"],
            ["o", "123"],
            ["q", "789"],
            ["testSnapshotGasLastCallGroupName", "45084"],
            ["testSnapshotGasSection", "5857390"],
            ["testSnapshotGasSectionGroupName", "5857820"],
            ["x", "123"],
            ["z", "789"],
          ]),
        ],
        [
          "GasSnapshotTest",
          new Map([
            ["a", "123"],
            ["b", "456"],
            ["c", "789"],
            ["d", "123"],
            ["e", "456"],
            ["f", "789"],
            ["testAssertGasExternal", "50265"],
            ["testAssertGasInternalA", "22052"],
            ["testAssertGasInternalB", "1021"],
            ["testAssertGasInternalC", "1020"],
            ["testAssertGasInternalD", "20921"],
            ["testAssertGasInternalE", "1021"],
            ["testSnapshotGasLastCallName", "45084"],
            ["testSnapshotGasSection", "5857390"],
            ["testSnapshotGasSectionName", "5857630"],
          ]),
        ],
      ])
    );
  });

  describe("InternalExpectRevert", async function () {
    it("allowInternalExpectRevert is true", async function () {
      const { totalTests, failedTests } = await testContext.runTestsWithStats(
        "InternalExpectRevertTest",
        {
          allowInternalExpectRevert: true,
        },
        L1_CHAIN_TYPE
      );

      assert.equal(totalTests, 1);
      assert.equal(failedTests, 0);
    });

    it("allowInternalExpectRevert default", async function () {
      const { totalTests, failedTests, stackTraces } =
        await testContext.runTestsWithStats(
          "InternalExpectRevertTest",
          undefined,
          L1_CHAIN_TYPE
        );

      assert.equal(totalTests, 1);
      assert.equal(failedTests, 1);

      const stackTrace = stackTraces.get("testInternalExpectRevert()");
      assert.equal(
        stackTrace?.reason,
        "call didn't revert at a lower depth than cheatcode call depth"
      );
    });
  });

  // Test that test suite results are returned in the order of completion and immediately after they're done.
  it("StreamingResults", async function () {
    const chainType = L1_CHAIN_TYPE;
    const testSuites = [
      "FirstReturnTest",
      "SecondReturnTest",
      "ThirdReturnTest",
    ];
    const testSuiteIds = testContext.matchingTests(new Set(testSuites));
    const config = testContext.defaultConfig(chainType);

    const results: {
      testResults: { testSuiteResult: SuiteResult; time: bigint }[];
      start: bigint;
    } = await new Promise((resolve, reject) => {
      const testResults: { testSuiteResult: SuiteResult; time: bigint }[] = [];
      const start = process.hrtime.bigint();
      testContext.edrContext
        .runSolidityTests(
          chainType,
          testContext.artifacts,
          testSuiteIds,
          config,
          testContext.tracingConfig,
          (testSuiteResult) => {
            testResults.push({
              testSuiteResult,
              time: process.hrtime.bigint(),
            });
            // Test timeout will handle it if this is never hit
            if (testResults.length == testSuiteIds.length) {
              resolve({ testResults, start });
            }
          }
        )
        .catch(reject);
    });
    const elapsed = process.hrtime.bigint() - results.start;

    assert.equal(results.testResults.length, 3);

    assert(
      Number(results.testResults[0].time) / Number(elapsed) > 2,
      `Time for first test is not more than 2x of starting test execution: first test ${results.testResults[0].time} vs prevTime ${elapsed}`
    );

    for (let i = 0; i < results.testResults.length; i++) {
      const suiteResult = results.testResults[i].testSuiteResult;

      assert.equal(suiteResult.id.name, testSuites[i]);
      assert.equal(suiteResult.testResults.length, 1);
      assert.equal(suiteResult.testResults[0].status, TestStatus.Success);

      if (i > 0) {
        const time = results.testResults[i].time - results.start;
        const prevTime = results.testResults[i - 1].time - results.start;
        assert(
          Number(time) / Number(prevTime) > 2,
          `Time for test ${i} is not greater than 2x: time ${time} vs prevTime ${prevTime}`
        );
      }
    }
  });

  it("ExpectRevertError", async function () {
    const { totalTests, failedTests, stackTraces } =
      await testContext.runTestsWithStats("ExpectRevertErrorTest");

    assert.equal(failedTests, 3);
    assert.equal(totalTests, 3);

    assertStackTraces(
      stackTraces.get("testFunctionDoesntRevertAsExpected()"),
      "next call did not revert as expected",
      [
        {
          contract: "ExpectRevertErrorTest",
          function: "testFunctionDoesntRevertAsExpected",
          line: 19, // foo.f();
          message: "next call did not revert as expected",
        },
      ]
    );

    assertStackTraces(
      stackTraces.get("testFunctionRevertsWithDifferentMessage()"),
      "Error != expected error: revert with a different message != expected message",
      [
        {
          contract: "ExpectRevertErrorTest",
          function: "testFunctionRevertsWithDifferentMessage",
          line: 25, // foo.g();
          message:
            "Error != expected error: revert with a different message != expected message",
        },
      ]
    );

    assertStackTraces(
      stackTraces.get("testFunctionRevertCountMismatch()"),
      "next call did not revert as expected",
      [
        {
          contract: "ExpectRevertErrorTest",
          function: "testFunctionRevertCountMismatch",
          line: 32, // foo.f();
          message: "next call did not revert as expected",
        },
      ]
    );
  });
});
