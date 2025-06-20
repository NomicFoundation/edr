import { createRequire } from "module";
import assert from "node:assert/strict";
import { test } from "node:test";
import path from "path";
import { dirName } from "@nomicfoundation/edr-helpers";

// The benchmarks assume that the EDR version used by
// Hardhat is the one in the workspace, instead of the one installed
// from npm. This test checks that this is the case.
// eslint-disable-next-line @typescript-eslint/no-floating-promises
test("uses the workspace version of EDR", function () {
  const require = createRequire(import.meta.url);
  const hardhatPath = require.resolve("hardhat");

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
});
