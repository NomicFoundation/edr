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

// Rebuild the corpus copy (see maybe-build.js) from scratch: `contracts/` is
// gitignored and is hardhat's sources dir, so a stray leftover .sol would be
// silently compiled into the committed fixtures.
const contractsDir = resolve(sweepRoot, "contracts");
rmSync(contractsDir, { recursive: true, force: true });
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
  (name) => name.endsWith(".json") && !name.endsWith(".output.json")
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

// Keep only what the Rust tests read: the scenarios source plus its
// inheritance closure (the build model resolves inherited functions through
// the BASE contracts' ASTs via `linearizedBaseContracts`). This filter is a
// size optimization, not the correctness point — sufficiency is enforced in
// `edr_solidity` by `scenarios_fixture_satisfies_contract_metadata_extraction`,
// which runs metadata extraction over the committed fixture. What this drops
// (console/safeconsole/Vm ASTs, ~35 MB) is read by nothing.
const allSources = buildInfoOutput.output.sources;
const allContracts = buildInfoOutput.output.contracts;
if (
  allSources[scenariosSource] === undefined ||
  allContracts[scenariosSource] === undefined
) {
  throw new Error(`compile output has no section for ${scenariosSource}`);
}
const baseContractIds = new Set(
  allSources[scenariosSource].ast.nodes
    .filter((node) => node.nodeType === "ContractDefinition")
    .flatMap((node) => node.linearizedBaseContracts)
);
const keptFiles = new Set(
  Object.entries(allSources)
    .filter(
      ([file, source]) =>
        file === scenariosSource ||
        source.ast.nodes?.some(
          (node) =>
            node.nodeType === "ContractDefinition" &&
            baseContractIds.has(node.id)
        )
    )
    .map(([file]) => file)
);
const pickKept = (map) =>
  Object.fromEntries(
    Object.entries(map).filter(([file]) => keptFiles.has(file))
  );
const output = {
  contracts: pickKept(allContracts),
  sources: pickKept(allSources),
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

const changed = execSync(
  "git status --porcelain -- crates/edr_solidity/fixtures/solx_compiler_input_scenarios.json crates/edr_solidity/fixtures/solx_compiler_output_scenarios.json",
  { cwd: repoRoot, encoding: "utf8" }
).trim();
console.log(
  changed === ""
    ? "[regen-fixtures] committed fixtures already match — nothing to re-pin."
    : "[regen-fixtures] fixtures changed — re-pin the Rust tests that assert on them; `cargo test -p edr_solidity` prints the new guard hash (see the README)."
);
