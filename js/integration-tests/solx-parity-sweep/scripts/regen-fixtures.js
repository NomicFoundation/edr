// Regenerates the committed scenarios fixture pair in
// `crates/edr_solidity/fixtures/` from a fresh solx-profile compile of this
// project. See the README's "Scenarios fixture provenance" section for the
// invariants this flow maintains and for what to re-pin afterwards.

import { execSync } from "node:child_process";
import {
  copyFileSync,
  mkdirSync,
  readdirSync,
  readFileSync,
  rmSync,
  writeFileSync,
} from "node:fs";
import { resolve } from "node:path";

try {
  await import("@nomicfoundation/hardhat-solx");
} catch {
  console.error(
    "[regen-fixtures] @nomicfoundation/hardhat-solx is not installed — the fixtures must be compiled by solx."
  );
  process.exit(1);
}

const sweepRoot = resolve(import.meta.dirname, "..");
const repoRoot = resolve(sweepRoot, "..", "..", "..");
const fixturesDir = resolve(repoRoot, "crates/edr_solidity/fixtures");
const buildInfoDir = resolve(sweepRoot, "artifacts", "build-info");
const scenariosSource = "project/contracts/Scenarios.t.sol";

// Refresh the pretest copy of the corpus (see maybe-build.js) so the compile
// picks up the current fixture source even when no test run preceded it.
const contractsDir = resolve(sweepRoot, "contracts");
mkdirSync(contractsDir, { recursive: true });
copyFileSync(
  resolve(fixturesDir, "sources", "Scenarios.t.sol"),
  resolve(contractsDir, "Scenarios.t.sol")
);

// Start from an empty build-info dir so exactly one fresh pair exists below.
rmSync(buildInfoDir, { recursive: true, force: true });
execSync("pnpm hardhat compile --build-profile solx", {
  cwd: sweepRoot,
  stdio: "inherit",
});

const ids = readdirSync(buildInfoDir).filter(
  (name) => !name.endsWith(".output.json")
);
if (ids.length !== 1) {
  throw new Error(`expected exactly one build info, found: ${ids}`);
}
const buildInfo = JSON.parse(
  readFileSync(resolve(buildInfoDir, ids[0]), "utf8")
);
const buildInfoOutput = JSON.parse(
  readFileSync(
    resolve(buildInfoDir, ids[0].replace(/\.json$/, ".output.json")),
    "utf8"
  )
);

// The input names every source and setting of the compile; the source text
// stays out — `Scenarios.t.sol` is spliced in at test time and the forge-std
// text is never read.
const input = buildInfo.input;
for (const source of Object.values(input.sources)) {
  source.content = "";
}

// Keep only the section the Rust tests read; the forge-std artifacts are
// ~40 MB they never touch.
const output = {
  contracts: {
    [scenariosSource]: buildInfoOutput.output.contracts[scenariosSource],
  },
  sources: {
    [scenariosSource]: buildInfoOutput.output.sources[scenariosSource],
  },
};

for (const [name, data] of [
  ["solx_compiler_input_scenarios.json", input],
  ["solx_compiler_output_scenarios.json", output],
]) {
  writeFileSync(
    resolve(fixturesDir, name),
    JSON.stringify(data, null, 2) + "\n"
  );
  console.log(`[regen-fixtures] wrote ${resolve(fixturesDir, name)}`);
}

console.log(
  "[regen-fixtures] if the fixtures changed, re-pin the Rust tests that assert on them — `cargo test -p edr_solidity` prints the new guard hash (see the README)."
);
