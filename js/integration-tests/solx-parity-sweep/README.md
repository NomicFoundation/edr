# solx-parity-sweep

Integration test that asserts EDR renders **the same Solidity stack trace** for a contract built with `solx` as for the same contract built with `solc` — across the revert/panic scenarios in `contracts/Scenarios.t.sol`.

## What it does

`test/sweep.ts` runs `hardhat test` twice (once with the `default` build profile = solc, once with the `solx` profile), parses the failing-test trace blocks from each run, and asserts per scenario that:

1. `Error:` reasons match.
2. Frame counts match.
3. Each frame's `Contract.function` location and `file:line` match.

## Pinned divergences

A small set of scenarios diverge from solc today and are pinned to their current solx output via `scenariosDivergingFromSolc` in `test/sweep.ts`. A golden mismatch means solx changed: either remove the entry (improvement) or update the pinned shape (regression).

| Scenario | Why it diverges |
| --- | --- |
| `InlineAssemblyRevertTest` | solx omits `.debug_line` rows for assembly opcodes; bottom frame falls back to the function decl line. |
| `InvalidOpcodeTest` | Same as inline-assembly: function decl line instead of statement line. |
| `InternalRecurseTest` | solx's optimizer fully unrolls 3-deep self-recursion; inlined frames collapse. |
| `BareModifierRevertTest` | A modifier's bare `revert()` is unmapped in the DWARF and returns no data; every revert heuristic misses and the trace collapses to the test's own call site. Fix queued in [#1552](https://github.com/NomicFoundation/edr/pull/1552). |

`MutualRecursionTest` and `NestedModifierRevertTest` were previously pinned but reached full parity with the debug info emitted by the merged `hardhat-solx` plugin, so they run under the strict parity check.

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

The script (`scripts/regen-fixtures.js`) wipes `artifacts/build-info`, compiles the solx profile, and writes the extracted pair; the committed fixtures are byte-for-byte its output. If it reports changes, re-pin the Rust tests that assert on the fixture **in the same commit** — `cargo test -p edr_solidity` prints the new guard hash, and the PC-anchored tests may need new offsets.

Details, for when a regeneration surprises you:

- What the compile produces depends on the toolchain of the day: hardhat-solx's compilation settings (e.g. its explicit `-O1` optimizer default) and the solx release its version map selects (`SOLIDITY_TO_SOLX_VERSION_MAP`; `0.8.34` → `0.1.4` today, with a bump to `0.1.6` planned). If the fixtures change when you didn't touch the corpus, one of those moved.
- The **output** is filtered down to `Scenarios.t.sol`'s own section — the only one the tests read (bytecode, DWARF `debugInfo` and ASTs); the compiled forge-std artifacts are ~40 MB the tests never touch.
- The **input** names every source of the compile and carries its settings, with `content` blank: `Scenarios.t.sol`'s text is spliced in at test time from `fixtures/sources/` (the same file this project compiles), and the forge-std text is never needed by the tests.
- solx embeds the build directory in the DWARF ([solx#594](https://github.com/NomicFoundation/solx/issues/594)), so `debugInfo` bytes differ across checkouts even on identical toolchains.
