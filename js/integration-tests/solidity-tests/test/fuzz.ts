import chai, { assert, expect } from "chai";
import chaiAsPromised from "chai-as-promised";
import { TestContext } from "./testContext";
import fs from "node:fs/promises";
import { FuzzConfigArgs } from "@nomicfoundation/edr";

chai.use(chaiAsPromised);

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

    // The fuzz failure directory should not be created if we don't set the directory
    await assert.isRejected(fs.stat(failureDir));

    const result2 = await testContext.runTestsWithStats("OverflowTest", {
      fuzz: {
        failurePersistDir: failureDir,
      },
    });
    assert.equal(result2.failedTests, 1);
    assert.equal(result2.totalTests, 1);

    // The fuzz failure directory should now be created
    await assert.isFulfilled(fs.stat(failureDir));
  });

  // One test as steps should be sequential
  it("FailingInvariant", async function () {
    const failureDir = testContext.invariantFailuresPersistDir;
    const defaultConfig = {
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
        invariant: defaultConfig,
      }
    );
    assert.equal(result1.failedTests, 1);
    assert.equal(result1.totalTests, 1);

    // The invariant failure directory should not be created if we don't set the directory
    await assert.isRejected(fs.stat(failureDir));

    const firstStart = performance.now();
    const result2 = await testContext.runTestsWithStats(
      "FailingInvariantTest",
      {
        invariant: {
          ...defaultConfig,
          failurePersistDir: failureDir,
        },
      }
    );
    const firstDuration = performance.now() - firstStart;
    assert.equal(result2.failedTests, 1);
    assert.equal(result2.totalTests, 1);

    // The invariant failure directory should now be created
    await assert.isFulfilled(fs.stat(failureDir));

    const secondStart = performance.now();
    const result3 = await testContext.runTestsWithStats(
      "FailingInvariantTest",
      {
        invariant: {
          ...defaultConfig,
          failurePersistDir: failureDir,
        },
      }
    );
    assert.equal(result2.failedTests, 1);
    assert.equal(result2.totalTests, 1);
    const secondDuration = performance.now() - secondStart;

    // The invariant failure directory should still be there
    await assert.isFulfilled(fs.stat(failureDir));
    // The second run should be faster than the first because it should use the persisted failure.
    assert.isBelow(secondDuration, firstDuration * 0.25);
  });
});
