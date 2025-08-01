import assert from "node:assert/strict";
import { before, describe, it } from "node:test";
import { TestContext } from "./testContext.js";
import { L1_CHAIN_TYPE, FsAccessPermission } from "@nomicfoundation/edr";
import { runAllSolidityTests } from "@nomicfoundation/edr-helpers";

describe("Fs permissions tests", () => {
  let testContext: TestContext;

  before(async () => {
    testContext = await TestContext.setup();
  });

  it("file + directory permissions", async function () {
    const results = await runAllSolidityTests(
      testContext.edrContext,
      L1_CHAIN_TYPE,
      testContext.artifacts,
      testContext.matchingTest("FsPermissionsTest"),
      testContext.tracingConfig,
      {
        ...testContext.defaultConfig(),
        allowInternalExpectRevert: true,
        fsPermissions: [
          {
            access: FsAccessPermission.ReadFile,
            path: "./fixtures/File/read.txt",
          },
          {
            access: FsAccessPermission.DangerouslyReadWriteDirectory,
            path: "./fixtures/Dir",
          },
          {
            access: FsAccessPermission.ReadWriteFile,
            path: "./fixtures/File/write_file.txt",
          },
        ],
      }
    );

    assert.equal(results.length, 1);
    assert.equal(results[0].testResults.length, 3);
    assert.equal(results[0].testResults[0].status, "Success");
    assert.equal(results[0].testResults[1].status, "Success");
    assert.equal(results[0].testResults[2].status, "Success");
  });

  it("not allowed cli.js permission", async function () {
    const results = await runAllSolidityTests(
      testContext.edrContext,
      L1_CHAIN_TYPE,
      testContext.artifacts,
      testContext.matchingTest("FsNotAllowedPermissionsTest"),
      testContext.tracingConfig,
      {
        ...testContext.defaultConfig(),
        allowInternalExpectRevert: true,
        fsPermissions: [
          {
            access: FsAccessPermission.DangerouslyReadWriteDirectory,
            path: "./fixtures",
          },
        ],
      }
    );

    assert.equal(results.length, 1);
    assert.equal(results[0].testResults.length, 2);
    assert.equal(results[0].testResults[0].status, "Success");
    assert.equal(results[0].testResults[1].status, "Success");
  });
});
