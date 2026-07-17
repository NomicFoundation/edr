// Compute the sentinel package versions for the Hardhat 3 regression benchmark
// and append them to $GITHUB_ENV.
//
// Requires the env vars EDR_REF and GITHUB_ENV, and a working directory at the
// EDR repo root (with Hardhat checked out under ./hardhat).

const fs = require("node:fs");
const path = require("node:path");

// EDR's sentinel is a `-local.<sha>` prerelease. It's only ever consumed as an
// exact pin (hardhat's `dependencies.@nomicfoundation/edr` and the platform-
// package wiring), which is not subject to semver range rules — so a prerelease
// is fine and keeps the benchmarked commit traceable in the version string.
function edrVersion(edrBaseVersion, shortSha) {
  return `${edrBaseVersion}-local.${shortSha}`;
}

// Hardhat's sentinel must be a *release* version (no prerelease tag). The e2e
// harness pins each scenario's `hardhat` dependency to it, and scenarios pull
// Hardhat plugins whose `peerDependencies` use ranges like `hardhat@^3.8.0`.
// node-semver excludes prereleases from such ranges.
//
// It must also be strictly ahead of the *last npm release*, not just of the
// checked-out repo's version: the harness's publish step patch-bumps any
// workspace package whose version doesn't exceed its last release, which would
// desync the published version from this prediction and fail the workflow's
// "Validate scenarios used the local EDR build" step. The Hardhat checkout can
// lag npm (its package.json only catches up when a release lands on the
// benchmarked ref), so floor the sentinel at the published version before
// bumping.
function hardhatVersion(hardhatBaseVersion, lastPublishedVersion) {
  let core = parseCore(hardhatBaseVersion);
  if (lastPublishedVersion !== undefined) {
    const published = parseCore(lastPublishedVersion);
    if (compareCores(core, published) < 0) {
      core = published;
    }
  }
  const [major, minor, patch] = core;
  return `${major}.${minor}.${patch + 1}`;
}

// Parses `major.minor.patch` out of a semver string, dropping any prerelease
// tag or build metadata.
function parseCore(version) {
  const core = version.split("+")[0].split("-")[0];
  const parts = core.split(".").map(Number);
  if (parts.length !== 3 || !parts.every(Number.isInteger)) {
    throw new Error(`Unparseable Hardhat version: ${version}`);
  }
  return parts;
}

function compareCores(a, b) {
  for (let i = 0; i < 3; i++) {
    if (a[i] !== b[i]) return a[i] - b[i];
  }
  return 0;
}

// The latest release of `pkg`, pinned to the public npm registry on purpose:
// the sentinel must be computed against the same "last release" the e2e
// harness's publish step compares to, so it must not be silently repointed by
// npm configuration (an `.npmrc`, `npm_config_registry`, …). The benchmark's
// Verdaccio isn't running yet when the sentinels are computed.
async function npmLatestVersion(pkg) {
  const url = `https://registry.npmjs.org/${pkg}/latest`;
  const response = await fetch(url);
  if (!response.ok) {
    throw new Error(`GET ${url} failed: ${response.status}`);
  }
  const { version } = await response.json();
  if (!version) {
    throw new Error(`GET ${url} returned no version`);
  }
  return version;
}

function readVersion(pkgJsonPath) {
  return JSON.parse(fs.readFileSync(pkgJsonPath, "utf8")).version;
}

async function main() {
  const { EDR_REF, GITHUB_ENV } = process.env;
  for (const [name, value] of Object.entries({ EDR_REF, GITHUB_ENV })) {
    if (!value) throw new Error(`${name} is not set`);
  }

  const cwd = process.cwd();
  const versions = {
    EDR_VER: edrVersion(
      readVersion(path.join(cwd, "crates/edr_napi/package.json")),
      EDR_REF.slice(0, 12)
    ),
    HH_VER: hardhatVersion(
      readVersion(path.join(cwd, "hardhat/packages/hardhat/package.json")),
      await npmLatestVersion("hardhat")
    ),
  };

  const lines = Object.entries(versions).map(([k, v]) => `${k}=${v}`);
  fs.appendFileSync(GITHUB_ENV, lines.join("\n") + "\n");
  for (const line of lines) console.log(line);
}

module.exports = { edrVersion, hardhatVersion };

if (require.main === module) {
  main().catch((error) => {
    console.error(error);
    process.exitCode = 1;
  });
}
