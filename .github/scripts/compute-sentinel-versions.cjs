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
function hardhatVersion(hardhatBaseVersion) {
  const core = hardhatBaseVersion.split("+")[0].split("-")[0];
  const [major, minor, patch] = core.split(".").map(Number);
  if (![major, minor, patch].every(Number.isInteger)) {
    throw new Error(`Unparseable Hardhat version: ${hardhatBaseVersion}`);
  }
  return `${major}.${minor}.${patch + 1}`;
}

function readVersion(pkgJsonPath) {
  return JSON.parse(fs.readFileSync(pkgJsonPath, "utf8")).version;
}

function main() {
  const { EDR_REF, GITHUB_ENV } = process.env;
  for (const [name, value] of Object.entries({ EDR_REF, GITHUB_ENV })) {
    if (!value) throw new Error(`${name} is not set`);
  }

  const cwd = process.cwd();
  const versions = {
    EDR_VER: edrVersion(
      readVersion(path.join(cwd, "crates/edr_napi/package.json")),
      EDR_REF.slice(0, 12),
    ),
    HH_VER: hardhatVersion(
      readVersion(path.join(cwd, "hardhat/packages/hardhat/package.json")),
    ),
  };

  const lines = Object.entries(versions).map(([k, v]) => `${k}=${v}`);
  fs.appendFileSync(GITHUB_ENV, lines.join("\n") + "\n");
  for (const line of lines) console.log(line);
}

module.exports = { edrVersion, hardhatVersion };

if (require.main === module) {
  main();
}
