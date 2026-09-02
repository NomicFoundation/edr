// Run with Node's built-in test runner (no extra dependencies):
//   node --test .github/scripts/compute-sentinel-versions.test.ts

import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { dirname, join } from "node:path";
import test from "node:test";
import { fileURLToPath } from "node:url";

import { edrVersion, hardhatVersion } from "./compute-sentinel-versions.ts";

const SCRIPT = join(
  dirname(fileURLToPath(import.meta.url)),
  "compute-sentinel-versions.ts"
);

// Run the script as the workflow does. The env guards run before any network
// access, so these stay offline.
function run(env: Record<string, string>) {
  return spawnSync(process.execPath, [SCRIPT], {
    encoding: "utf8",
    env: { ...process.env, ...env },
  });
}

test("edrVersion appends a -local.<sha> prerelease", () => {
  assert.equal(
    edrVersion("0.12.1", "dfeb0f9b95f9"),
    "0.12.1-local.dfeb0f9b95f9"
  );
});

test("hardhatVersion bumps the patch of a release version", () => {
  assert.equal(hardhatVersion("3.9.0"), "3.9.1");
  assert.equal(hardhatVersion("2.0.0"), "2.0.1");
});

test("hardhatVersion strips a prerelease tag, then bumps to a release", () => {
  // The result must be a release (no `-`) so `^3.x` peer ranges match it.
  assert.equal(hardhatVersion("3.10.0-next.1"), "3.10.1");
  assert.equal(hardhatVersion("3.9.0-edr.dfeb0f9b95f9"), "3.9.1");
});

test("hardhatVersion strips build metadata", () => {
  assert.equal(hardhatVersion("3.9.5+build.7"), "3.9.6");
});

test("hardhatVersion floors the sentinel at the last npm release", () => {
  // The repo lags npm: a release shipped that the benchmarked ref hasn't
  // caught up to. Bumping only the repo version would collide with the
  // published release, so the e2e harness would re-bump on top and desync the
  // published version from the sentinel (observed with hardhat 3.9.1 released
  // while the checkout was at 3.9.0: the harness published 3.9.2).
  assert.equal(hardhatVersion("3.9.0", "3.9.1"), "3.9.2");
  assert.equal(hardhatVersion("3.9.0", "4.0.0"), "4.0.1");
});

test("hardhatVersion keeps the repo version when it is ahead of npm", () => {
  assert.equal(hardhatVersion("3.10.0", "3.9.1"), "3.10.1");
  assert.equal(hardhatVersion("3.9.1", "3.9.1"), "3.9.2");
});

test("hardhatVersion strips a prerelease tag from the published version", () => {
  assert.equal(hardhatVersion("3.9.0", "3.9.1-beta.1"), "3.9.2");
});

test("hardhatVersion throws on an unparseable version", () => {
  assert.throws(() => hardhatVersion("not.a.version"), /Unparseable/);
  assert.throws(() => hardhatVersion("3.x"), /Unparseable/);
  assert.throws(() => hardhatVersion("3.9.0.1"), /Unparseable/);
  assert.throws(() => hardhatVersion("3.9.0", "3.x"), /Unparseable/);
});

// The entrypoint is the whole contract with the workflow: it must run when
// executed by `node`, and it must fail loudly rather than leave $GITHUB_ENV
// unwritten — an unset EDR_VER only surfaces ~40 minutes later, after the Rust
// and Hardhat builds, as an "unbound variable" in a later step.
test("CLI exits 1 when EDR_REF is not set", () => {
  const result = run({ EDR_REF: "", GITHUB_ENV: "/dev/null" });

  assert.equal(result.status, 1);
  assert.match(result.stderr, /EDR_REF is not set/);
});

test("CLI exits 1 when GITHUB_ENV is not set", () => {
  const result = run({ EDR_REF: "deadbeefcafe", GITHUB_ENV: "" });

  assert.equal(result.status, 1);
  assert.match(result.stderr, /GITHUB_ENV is not set/);
});
