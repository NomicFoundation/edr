// Run with Node's built-in test runner (no extra dependencies):
//   node --test .github/scripts/

const test = require("node:test");
const assert = require("node:assert/strict");

const {
  edrVersion,
  hardhatVersion,
} = require("./compute-sentinel-versions.cjs");

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
  assert.throws(() => hardhatVersion("3.9.0", "3.x"), /Unparseable/);
});
