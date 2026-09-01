// Compute the sentinel package versions for the Hardhat 3 regression benchmark
// and append them to $GITHUB_ENV.
//
// Requires the env vars EDR_REF and GITHUB_ENV, and a working directory at the
// EDR repo root (with Hardhat checked out under ./hardhat).
//
// See README.md for the conventions these scripts follow.

import { appendFileSync, readFileSync } from "node:fs";
import { join } from "node:path";

/** A semver `major.minor.patch`, with any prerelease or build metadata dropped. */
type SemverCore = [number, number, number];

// EDR's sentinel is a `-local.<sha>` prerelease. It's only ever consumed as an
// exact pin (hardhat's `dependencies.@nomicfoundation/edr` and the platform-
// package wiring), which is not subject to semver range rules — so a prerelease
// is fine and keeps the benchmarked commit traceable in the version string.
export function edrVersion(edrBaseVersion: string, shortSha: string): string {
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
export function hardhatVersion(
  hardhatBaseVersion: string,
  lastPublishedVersion?: string
): string {
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
function parseCore(version: string): SemverCore {
  // `split` with a non-empty separator always yields at least one element, so
  // both `??` fallbacks are unreachable; they satisfy noUncheckedIndexedAccess.
  const withoutBuildMetadata = version.split("+", 1)[0] ?? version;
  const core = withoutBuildMetadata.split("-", 1)[0] ?? withoutBuildMetadata;
  const parts = core.split(".").map(Number);
  const [major, minor, patch] = parts;

  if (
    parts.length !== 3 ||
    major === undefined ||
    minor === undefined ||
    patch === undefined ||
    !parts.every(Number.isInteger)
  ) {
    throw new Error(`Unparseable Hardhat version: ${version}`);
  }

  return [major, minor, patch];
}

function compareCores(a: SemverCore, b: SemverCore): number {
  return a[0] - b[0] || a[1] - b[1] || a[2] - b[2];
}

// The latest release of `pkg`, pinned to the public npm registry on purpose:
// the sentinel must be computed against the same "last release" the e2e
// harness's publish step compares to, so it must not be silently repointed by
// npm configuration (an `.npmrc`, `npm_config_registry`, …). The benchmark's
// Verdaccio isn't running yet when the sentinels are computed.
async function npmLatestVersion(pkg: string): Promise<string> {
  const url = `https://registry.npmjs.org/${pkg}/latest`;
  const response = await fetch(url);

  if (!response.ok) {
    throw new Error(`GET ${url} failed: ${response.status}`);
  }

  const { version } = (await response.json()) as { version?: unknown };

  if (typeof version !== "string" || version === "") {
    throw new Error(`GET ${url} returned no version`);
  }

  return version;
}

function readVersion(pkgJsonPath: string): string {
  const parsed: unknown = JSON.parse(readFileSync(pkgJsonPath, "utf8"));
  const version =
    typeof parsed === "object" && parsed !== null && "version" in parsed
      ? parsed.version
      : undefined;

  if (typeof version !== "string" || version === "") {
    throw new Error(`${pkgJsonPath} has no version`);
  }

  return version;
}

async function main(): Promise<void> {
  const { EDR_REF, GITHUB_ENV } = process.env;

  if (EDR_REF === undefined || EDR_REF === "") {
    throw new Error("EDR_REF is not set");
  }
  if (GITHUB_ENV === undefined || GITHUB_ENV === "") {
    throw new Error("GITHUB_ENV is not set");
  }

  const cwd = process.cwd();
  const versions = {
    EDR_VER: edrVersion(
      readVersion(join(cwd, "crates/edr_napi/package.json")),
      EDR_REF.slice(0, 12)
    ),
    HH_VER: hardhatVersion(
      readVersion(join(cwd, "hardhat/packages/hardhat/package.json")),
      await npmLatestVersion("hardhat")
    ),
  };

  const lines = Object.entries(versions).map(([k, v]) => `${k}=${v}`);
  // Append, never truncate: earlier steps in this job wrote E2E_CLONE_DIR and
  // the package-manager cache paths here.
  appendFileSync(GITHUB_ENV, lines.join("\n") + "\n");
  for (const line of lines) console.log(line);
}

if (import.meta.main) {
  main().catch((error: unknown) => {
    console.error(error);
    process.exitCode = 1;
  });
}
