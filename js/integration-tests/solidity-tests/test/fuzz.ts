import assert from "node:assert/strict";
import { before, describe, it } from "node:test";
import {
  assertImpureCheatcode,
  assertStackTraces,
  TestContext,
} from "./testContext.js";
import fs from "node:fs/promises";
import { existsSync } from "node:fs";
import {
  EdrContext,
  FuzzTestKind,
  InvariantTestKind,
  L1_CHAIN_TYPE,
  l1SolidityTestRunnerFactory,
} from "@nomicfoundation/edr";
import { runAllSolidityTests } from "@nomicfoundation/edr-helpers";

describe("Fuzz and invariant testing", function () {
  let testContext: TestContext;

  before(async function () {
    testContext = await TestContext.setup();
  });

  it("FailingFuzz", async function () {
    const failureDir = testContext.fuzzFailuresPersistDir;

    // Remove invariant config failure directory to make sure it's created fresh.
    await fs.rm(failureDir, {
      recursive: true,
      force: true,
    });

    const result1 = await testContext.runTestsWithStats("OverflowTest");
    assert.equal(result1.failedTests, 1);
    assert.equal(result1.totalTests, 1);

    assertStackTraces(
      result1.stackTraces.get("testFuzzAddWithOverflow(uint256,uint256)"),
      "arithmetic underflow or overflow",
      [
        { contract: "OverflowTest", function: "testFuzzAddWithOverflow" },
        { contract: "MyContract", function: "addWithOverflow" },
      ]
    );

    // The fuzz failure directory should not be created if we don't set the directory
    assert.ok(!existsSync(failureDir));

    const result2 = await testContext.runTestsWithStats("OverflowTest", {
      fuzz: {
        failurePersistDir: failureDir,
      },
    });
    assert.equal(result2.failedTests, 1);
    assert.equal(result2.totalTests, 1);

    // The fuzz failure directory should now be created
    assert.ok(existsSync(failureDir));
  });

  it("ImpureFuzzSetup", async function () {
    const result1 = await testContext.runTestsWithStats("ImpureFuzzSetup");
    assert.equal(result1.failedTests, 1);
    assert.equal(result1.totalTests, 1);

    const stackTrace = result1.stackTraces.get("setUp()");
    assertImpureCheatcode(stackTrace, "readFile");
  });

  it("ImpureFuzzTest", async function () {
    const result1 = await testContext.runTestsWithStats("ImpureFuzzTest");
    assert.equal(result1.failedTests, 1);
    assert.equal(result1.totalTests, 1);

    const stackTrace = result1.stackTraces.get(
      "testFuzzAddWithOverflow(uint256,uint256)"
    );
    assertImpureCheatcode(stackTrace, "unixTime");
  });

  it("FuzzFixture is not supported", async function () {
    const result = await testContext.runTestsWithStats("FuzzFixtureTest", {
      fuzz: {
        runs: 256,
        dictionaryWeight: 1,
        includeStorage: false,
        includePushBytes: false,
        seed: "0x7bb9ee74aaa2abe5e2ca8a116382a9f2ed70b651e70b430e1052eff52a74ffe3",
      },
    });
    assert.equal(result.failedTests, 0);
    assert.equal(result.totalTests, 1);
  });

  // One test as steps should be sequential
  it("FailingInvariant", async function () {
    const expectedReason = "assertion failed";
    const expectedStackTraces = [
      { contract: "FailingInvariantTest", function: "invariant" },
      {
        contract: "FailingInvariantTest",
        function: "assertEq",
      },
    ];
    const failureDir = testContext.invariantFailuresPersistDir;
    const invariantConfig = {
      runs: 256,
      depth: 15,
      // This is false by default, we just specify it here to make it obvious to the reader.
      failOnRevert: false,
    };

    // Remove invariant config failure directory to make sure it's created fresh.
    await fs.rm(failureDir, {
      recursive: true,
      force: true,
    });

    const result1 = await testContext.runTestsWithStats(
      "FailingInvariantTest",
      {
        invariant: invariantConfig,
      }
    );
    assert.equal(result1.failedTests, 1);
    assert.equal(result1.totalTests, 1);
    assertStackTraces(
      result1.stackTraces.get("invariant()"),
      expectedReason,
      expectedStackTraces
    );

    // The invariant failure directory should not be created if we don't set the directory
    assert.ok(!existsSync(failureDir));

    const [, results2] = await runAllSolidityTests(
      testContext.edrContext,
      L1_CHAIN_TYPE,
      testContext.artifacts,
      testContext.matchingTest("FailingInvariantTest"),
      testContext.tracingConfig,
      {
        ...testContext.defaultConfig(),
        invariant: {
          ...invariantConfig,
          failurePersistDir: failureDir,
        },
        fuzz: {
          seed: "100",
        },
      }
    );
    assert.equal(results2.length, 1);
    assert.equal(results2[0].testResults.length, 1);
    assert.equal(results2[0].testResults[0].status, "Failure");
    const invariantTestResult = results2[0].testResults[0]
      .kind as InvariantTestKind;
    // More than one call should be needed on a fresh invariant test.
    assert.ok(invariantTestResult.calls > 1n);
    const stackTrace2 = results2[0].testResults[0].stackTrace();
    assertStackTraces(
      {
        stackTrace: stackTrace2,
        reason: results2[0].testResults[0].reason,
      },
      expectedReason,
      expectedStackTraces
    );

    // The invariant failure directory should now be created
    assert.ok(existsSync(failureDir));

    const [, results3] = await runAllSolidityTests(
      testContext.edrContext,
      L1_CHAIN_TYPE,
      testContext.artifacts,
      testContext.matchingTest("FailingInvariantTest"),
      testContext.tracingConfig,
      {
        ...testContext.defaultConfig(),
        invariant: {
          ...invariantConfig,
          failurePersistDir: failureDir,
        },
      }
    );

    // The invariant failure directory should still be there
    assert.ok(existsSync(failureDir));
    assert.equal(results3.length, 1);
    assert.equal(results3[0].testResults.length, 1);
    assert.equal(results3[0].testResults[0].status, "Failure");
    const fuzzTestResult3 = results3[0].testResults[0].kind as FuzzTestKind;
    // The second time only one run should be needed, because the persisted failure is used.
    assert.equal(fuzzTestResult3.runs, 1n);
    const stackTrace3 = results3[0].testResults[0].stackTrace();
    assertStackTraces(
      {
        stackTrace: stackTrace3,
        reason: results3[0].testResults[0].reason,
      },
      "invariant replay failure",
      expectedStackTraces
    );
  });

  it("BuggyInvariant", async function () {
    const expectedReason = "one is not two";
    const expectedStackTraces = [
      { contract: "BuggyInvariantTest", function: "invariant" },
    ];

    const failureDir = testContext.invariantFailuresPersistDir;
    const invariantConfig = {
      runs: 256,
      depth: 15,
      // This is false by default, we just specify it here to make it obvious to the reader.
      failOnRevert: false,
    };

    // Remove invariant config failure directory to make sure it's created fresh.
    await fs.rm(failureDir, {
      recursive: true,
      force: true,
    });

    const result = await testContext.runTestsWithStats("BuggyInvariantTest", {
      invariant: invariantConfig,
    });
    assert.equal(result.failedTests, 1);
    assert.equal(result.totalTests, 1);
    assertStackTraces(
      result.stackTraces.get("invariant()"),
      expectedReason,
      expectedStackTraces
    );
  });

  it("ImpureInvariantTest", async function () {
    const invariantConfig = {
      runs: 256,
      depth: 15,
      // This is false by default, we just specify it here to make it obvious to the reader.
      failOnRevert: false,
    };

    const result = await testContext.runTestsWithStats("ImpureInvariantTest", {
      invariant: invariantConfig,
    });
    assert.equal(result.failedTests, 1);
    assert.equal(result.totalTests, 1);

    const stackTrace = result.stackTraces.get("invariant()");
    assertImpureCheatcode(stackTrace, "unixTime");
  });

  // Test that changing the source for a test invalidates persisted failures.
  it("InvariantSourceChange", async function () {
    const failureDir = testContext.invariantFailuresPersistDir;

    await fs.rm(failureDir, {
      recursive: true,
      force: true,
    });

    const invariantConfig = {
      runs: 256,
      depth: 15,
      failOnRevert: true,
    };

    const originalArtifacts = testContext.artifacts.filter((artifact) => {
      return artifact.id.source.endsWith("InvariantSourceChange.t.sol")
    });
    const originalTestArtifact = originalArtifacts.find((artifact) => {
      return artifact.id.name === "InvariantSourceChangeTest"
    })
    const originalHandlerArtifact = originalArtifacts.find((artifact) => {
      return artifact.id.name === "AssumeHandler"
    })
    assert.equal(originalArtifacts.length, 2);

    const [, originalResults] = await runAllSolidityTests(
      testContext.edrContext,
      L1_CHAIN_TYPE,
      originalArtifacts,
      [originalTestArtifact!.id],
      testContext.tracingConfig,
      {
        ...testContext.defaultConfig(),
        invariant: {
          ...invariantConfig,
          failurePersistDir: failureDir,
        },
        fuzz: {
          seed: "100",
        },
      }
    );
    assert.equal(originalResults.length, 1);
    assert.ok(originalResults[0].id.source.endsWith("InvariantSourceChange.t.sol"));
    assert.equal(originalResults[0].testResults.length, 1);
    assert.equal(originalResults[0].testResults[0].name, "invariant_assume()");
    assert.equal(originalResults[0].testResults[0].status, "Failure");
    assert.equal(originalResults[0].testResults[0].reason, "Invariant failure");


    const changedArtifacts = testContext.artifacts.filter((artifact) => {
      return artifact.id.source.endsWith("InvariantSourceChangeTwo.t.sol")
    });
    const changedTestArtifact = changedArtifacts.find((artifact) => {
      return artifact.id.name === "InvariantSourceChangeTest"
    })
    const changedHandlerArtifact = changedArtifacts.find((artifact) => {
      return artifact.id.name === "AssumeHandler"
    })
    originalTestArtifact!.contract = changedTestArtifact!.contract;
    originalHandlerArtifact!.contract = changedHandlerArtifact!.contract;

    const [, changedResults] = await runAllSolidityTests(
      testContext.edrContext,
      L1_CHAIN_TYPE,
      originalArtifacts,
      [originalTestArtifact!.id],
      testContext.tracingConfig,
      {
        ...testContext.defaultConfig(),
        invariant: {
          ...invariantConfig,
          failurePersistDir: failureDir,
          maxAssumeRejects: 10
        },
        fuzz: {
          seed: "100",
        },
      }
    );
    assert.equal(changedResults.length, 1);
    assert.ok(changedResults[0].id.source.endsWith("InvariantSourceChange.t.sol"));
    assert.equal(changedResults[0].testResults.length, 1);
    assert.equal(changedResults[0].testResults[0].name, "invariant_assume()");
    assert.equal(changedResults[0].testResults[0].status, "Failure");
    assert.equal(changedResults[0].testResults[0].reason, "`vm.assume` rejected too many inputs (10 allowed)");
  });

  it("ShouldRevertWithAssumeCode", async function () {
    const invariantConfig = {
      failOnRevert: true,
      maxAssumeRejects: 10,
    };

    const result = await testContext.runTestsWithStats("BalanceAssumeTest", {
      invariant: invariantConfig,
      fuzz: {
        seed: "100",
      },
    });
    assert.equal(result.failedTests, 1);
    assert.equal(result.totalTests, 1);

    const expectedReason = "`vm.assume` rejected too many inputs (10 allowed)";

    const stackTrace = result.stackTraces.get("invariant_balance()");
    assert.equal(stackTrace?.reason, expectedReason);
  });

  it("ShouldNotPanicIfNoSelectors", async function () {
    const result = await testContext.runTestsWithStats("NoSelectorTest");
    assert.equal(result.failedTests, 1);
    assert.equal(result.totalTests, 1);

    const expectedReason =
      "failed to set up invariant testing environment: No contracts to fuzz.";

    const stackTrace = result.stackTraces.get("invariant_panic()");
    assert.equal(stackTrace?.reason, expectedReason);
  });

  it("InvariantTestTarget", async function () {
    const invariantConfig = {
      runs: 5,
      depth: 5,
    };

    let result = await testContext.runTestsWithStats("InvariantTestNoTarget", {
      invariant: invariantConfig,
    });
    assert.equal(result.failedTests, 1);
    assert.equal(result.totalTests, 1);

    let expectedReason =
      "failed to set up invariant testing environment: No contracts to fuzz.";

    let stackTrace = result.stackTraces.get("invariant_check_count()");
    assert.equal(stackTrace?.reason, expectedReason);

    // Tests targetContract.
    result = await testContext.runTestsWithStats("InvariantTestTarget", {
      invariant: invariantConfig,
    });
    assert.equal(result.failedTests, 0);
    assert.equal(result.totalTests, 1);
  });

  it("InvariantTestTargetContractSelectors", async function () {
    const invariantConfig = {
      runs: 10,
      depth: 100,
    };

    // Only this selector should be targeted.
    const expectedSelector =
      "project/test-contracts/TargetContractSelectors.t.sol:InvariantTargetTestSelectors.foo";

    const result = await testContext.runTestsWithStats(
      "InvariantTargetTestSelectors",
      {
        invariant: invariantConfig,
        testPattern: "invariant",
      }
    );

    // Only invariant tests should run.
    assert.equal(result.failedTests, 0);
    assert.equal(result.totalTests, 4);

    const suiteResults = result.suiteResults[0];
    for (const testResult of suiteResults.testResults) {
      if ("metrics" in testResult.kind) {
        const metrics = testResult.kind.metrics;
        assert.equal(Object.keys(metrics).length, 1);
        assert(metrics[expectedSelector] !== undefined);
      } else {
        throw new Error(
          "The 'metrics' property does not exist on this test kind."
        );
      }
    }
  });

  it("InvariantTestTargetIncludeExcludeSelectors", async function () {
    const invariantConfig = {
      runs: 10,
      depth: 100,
    };

    const testCases = [
      {
        // Tests tagetSelector.
        testName: "InvariantTargetIncludeTest",
        expectedMetrics: [
          "project/test-contracts/TargetIncludeExcludeSelectors.t.sol:InvariantTargetIncludeTest.shouldInclude1",
          "project/test-contracts/TargetIncludeExcludeSelectors.t.sol:InvariantTargetIncludeTest.shouldInclude2",
        ],
      },
      {
        // Tests excludeSelector.
        testName: "InvariantTargetExcludeTest",
        expectedMetrics: [
          "project/test-contracts/TargetIncludeExcludeSelectors.t.sol:InvariantTargetExcludeTest.shouldInclude1",
          "project/test-contracts/TargetIncludeExcludeSelectors.t.sol:InvariantTargetExcludeTest.shouldInclude2",
        ],
      },
    ];

    for (const { testName, expectedMetrics } of testCases) {
      const result = await testContext.runTestsWithStats(testName, {
        invariant: invariantConfig,
      });
      assert.equal(result.failedTests, 0);
      assert.equal(result.totalTests, 1);

      const suiteResult = result.suiteResults[0];
      if ("metrics" in suiteResult.testResults[0].kind) {
        const metrics = suiteResult.testResults[0].kind.metrics;
        assert.equal(Object.keys(metrics).length, expectedMetrics.length);
        for (const metric of expectedMetrics) {
          assert(metrics[metric] !== undefined);
        }
      } else {
        throw new Error(
          "The 'metrics' property does not exist on this test kind."
        );
      }
    }
  });
});
