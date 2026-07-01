// The sweep skips itself when `@nomicfoundation/hardhat-solx` is not
// installed (it is an optional dependency). When the suite is going to skip,
// running the workspace's full `pnpm build:dev` from `pretest` is wasted CI
// time. Detect the optional-dep state up front and only run the build when
// it actually has work to do.
//
// TODO: once `@nomicfoundation/hardhat-solx` is published on npm, move it
// from `optionalDependencies` to `devDependencies` in package.json and
// delete this script — `pretest` can then just call `pnpm build:dev`
// directly.

import { execSync } from "node:child_process";
import { copyFileSync, mkdirSync } from "node:fs";
import { resolve } from "node:path";

let hardhatSolxAvailable = false;
try {
  await import("@nomicfoundation/hardhat-solx");
  hardhatSolxAvailable = true;
} catch {
  // optional dep missing — sweep will skip; nothing to build.
}

if (!hardhatSolxAvailable) {
  console.log(
    "[solx-parity-sweep] hardhat-solx not installed; skipping pretest build.",
  );
  process.exit(0);
}

const sweepRoot = resolve(import.meta.dirname, "..");
const repoRoot = resolve(sweepRoot, "..", "..", "..");

// Hardhat 3 refuses files outside the project root, so we copy the
// single-source-of-truth Scenarios.t.sol from `crates/edr_solidity/fixtures/`
// into the sweep's `contracts/` at pretest time. The destination is gitignored.
const fixturesDir = resolve(
  repoRoot,
  "crates/edr_solidity/fixtures/sources",
);
const contractsDir = resolve(sweepRoot, "contracts");
mkdirSync(contractsDir, { recursive: true });
copyFileSync(
  resolve(fixturesDir, "Scenarios.t.sol"),
  resolve(contractsDir, "Scenarios.t.sol"),
);

execSync("pnpm build:dev", { cwd: repoRoot, stdio: "inherit" });
