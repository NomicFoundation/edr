import { createRequire } from "module";
import assert from "node:assert/strict";
import { test } from "node:test";
import path from "path";
import { dirName } from "@nomicfoundation/edr-helpers";

// The provider scenario benchmarks assume that the EDR version used by
// Hardhat is the one in the workspace, instead of the one installed
// from npm. This test checks that this is the case.
function checkHardhatEdrVersion(hardhatPackageName: string) {
  const require = createRequire(import.meta.url);
  const hardhatPath = require.resolve(hardhatPackageName);

  const edrPath = require.resolve("@ignored/edr-optimism", {
    paths: [hardhatPath],
  });

  const expectedPath = path.resolve(
    dirName(import.meta.url),
    "..",
    "..",
    "..",
    "..",
    "..",
    "crates",
    "edr_napi",
    "index.js"
  );

  assert.equal(edrPath, expectedPath);
}

// False positive
// eslint-disable-next-line @typescript-eslint/no-floating-promises
test("Hardhat 2 uses the workspace version of EDR", function () {
  checkHardhatEdrVersion("hardhat2");
});

// False positive
// eslint-disable-next-line @typescript-eslint/no-floating-promises
test("Hardhat 3 uses the workspace version of EDR", function () {
  checkHardhatEdrVersion("hardhat");
});
