# solx-parity-sweep

Integration test that asserts EDR renders **the same Solidity stack trace** for a contract built with `solx` as for the same contract built with `solc` — across the revert/panic scenarios in `contracts/Scenarios.t.sol`.

## What it does

`test/sweep.ts` runs `hardhat test` twice (once with the `default` build profile = solc, once with the `solx` profile), parses the failing-test trace blocks from each run, and asserts per scenario that:

1. `Error:` reasons match.
2. Frame counts match.
3. Each frame's `Contract.function` location and `file:line` match.

## Pinned divergences

Scenarios that diverge from solc are pinned to solx's output via `scenariosDivergingFromSolc` in `test/sweep.ts`; every other scenario runs under the strict parity check. A golden mismatch means solx changed: either remove the entry (improvement) or update the pinned shape (regression). The pin set is specific to the solx release `hardhat-solx`'s version map selects.

| Scenario | Why it diverges |
| --- | --- |
| `InternalRecurseTest` | solx's optimizer fully unrolls 3-deep self-recursion; inlined frames collapse. |

## Current state

Not yet running in CI. The suite has `@nomicfoundation/hardhat-solx` as an `optionalDependencies` entry because that package is not yet on the public npm registry; without it the suite self-skips.

## Prerequisites

To run the sweep, a local build of `hardhat-solx` must be linked into this package.

```sh
# 1. Clone the hardhat monorepo (the plugin lives on main, under packages/hardhat-solx).
git clone https://github.com/NomicFoundation/hardhat.git
cd hardhat

# 2. Install + build the monorepo so packages/hardhat-solx/dist exists.
pnpm install
pnpm --filter @nomicfoundation/hardhat-solx build

# 3. Symlink the built plugin into this package's node_modules.
cd <edr-repo>/js/integration-tests/solx-parity-sweep
mkdir -p node_modules/@nomicfoundation
ln -s <path-to-hardhat-clone>/packages/hardhat-solx node_modules/@nomicfoundation/hardhat-solx
```

> Do not use `pnpm link` for step 3: with pnpm ≥ 9 it writes a machine-local `link:` dependency into the workspace root's `package.json`, `pnpm-workspace.yaml` and `pnpm-lock.yaml`, which must never be committed. The plain symlink has no side effects. Note that a `pnpm install` recreates `node_modules`, removing the symlink — re-create it afterwards.

## Running

```sh
pnpm install
pnpm test
```

The `pretest` step builds the workspace's `@nomicfoundation/edr` napi binary so the sweep runs against current EDR sources. With no `hardhat-solx` linked the suite self-skips quickly.

## Adding scenarios

Scenarios live in `crates/edr_solidity/fixtures/sources/Scenarios.t.sol` (copied into `contracts/` at pretest). To add one:

1. **Append** a target contract plus a failing forge test at the **end of the file** — never edit the existing content. The file is spliced at test time into a frozen compiled fixture whose AST and DWARF reference byte offsets into the source as it was when the fixture was generated; any edit inside that prefix desyncs the Rust tests built on it. The rule is enforced by `scenarios_source_is_append_only` in `crates/edr_solidity/src/debug_info/dwarf.rs`.
2. Run `pnpm test`. Scenario keys are discovered dynamically from the failing-test output, so the new scenario joins the strict parity check with no further wiring.
3. If solx matches solc: done. If it diverges: pin the solx output in `scenariosDivergingFromSolc` with a comment saying why it diverges and which direction a future golden break means (improvement → remove or shrink the entry; regression → investigate).

New scenarios exist only in the sweep until someone regenerates the compiled fixture — the Rust tests that share this file cannot see them, so provider-path coverage for the same shape needs a separate scenario in the regenerable `StackTraceScenarios.sol` fixture.

## Scenarios fixture provenance

The Rust tests in `edr_solidity` pair this project's scenario source with a committed compile of it: `crates/edr_solidity/fixtures/solx_compiler_{input,output}_scenarios.json`. To regenerate the pair:

```sh
pnpm regen-fixtures
```

The script (`scripts/regen-fixtures.js`) wipes `artifacts/build-info`, compiles the solx profile, and writes the extracted pair; the committed fixtures are byte-for-byte its output. If it reports changes, re-pin the Rust tests that assert on the fixture **in the same commit** — `cargo test -p edr_solidity` prints the new guard hash, and the PC-anchored tests may need new offsets. Validate **both** consumers: `cargo test -p edr_solidity` and `cargo test -p edr_provider --all-features --test main solx_stack_trace`.

Details, for when a regeneration surprises you:

- What the compile produces depends on the toolchain of the day: hardhat-solx's compilation settings (e.g. its explicit `-O1` optimizer default) and the solx release its version map selects (`SOLIDITY_TO_SOLX_VERSION_MAP`). If the fixtures change when you didn't touch the corpus, one of those moved — check which solx actually compiled the output via the bytecode's trailing CBOR `solcx` stamp (a locally linked hardhat-solx build can silently bring a different map).
- Finding the new offsets for the PC-anchored dwarf tests: next to the failing test, temporarily iterate `decode_deployed_for(...)`, filter for the opcode you're after (`REVERT`, `INVALID`), and print each candidate's `pc`, resolved line and inline call sites; run with `cargo test -p edr_solidity -- --nocapture` (the harness hides passing tests' output) and pick the instruction whose line matches the test's intent. Appending scenarios never moves sibling contracts' PCs — each contract is its own compilation unit — so this is only needed when the toolchain changes codegen.
- The **output** is filtered to `Scenarios.t.sol` **plus its inheritance closure** (the forge-std bases: `Test.sol`, `StdInvariant.sol`, …). EDR's build model resolves inherited functions through the base contracts' ASTs (`linearizedBaseContracts`), so `sources` and `contracts` must cover the same closure. The filter here is only a size optimization — sufficiency is enforced where the knowledge lives: `scenarios_fixture_satisfies_contract_metadata_extraction` in `edr_solidity` runs contract metadata extraction over the committed fixture, so an over-filtered regen fails `cargo test -p edr_solidity` directly. What is dropped (`console.sol`/`safeconsole.sol`/`Vm.sol` ASTs, ~35 MB) is read by nothing.
- The **input** names every source of the compile and carries its settings, with `content` blank: `Scenarios.t.sol`'s text is spliced in at test time from `fixtures/sources/` (the same file this project compiles), and the forge-std text is never needed by the tests.
- solx embeds the build directory in the DWARF ([solx#594](https://github.com/NomicFoundation/solx/issues/594)), so `debugInfo` bytes differ across checkouts even on identical toolchains.
