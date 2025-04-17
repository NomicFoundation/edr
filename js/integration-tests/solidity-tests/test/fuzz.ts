import assert from "node:assert/strict";
import { before, describe, it } from "node:test";
import {
  assertImpureCheatcode,
  assertStackTraces,
  TestContext,
} from "./testContext.js";
import fs from "node:fs/promises";
import { existsSync } from "node:fs";
import { FuzzTestKind, InvariantTestKind } from "@ignored/edr";
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

    const results2 = await runAllSolidityTests(
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

    const results3 = await runAllSolidityTests(
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
    console.log(fuzzTestResult3);
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
    const expectedReason = "revert: one is not two";
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
    console.log(result.stackTraces.get("invariant()"));
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
});
