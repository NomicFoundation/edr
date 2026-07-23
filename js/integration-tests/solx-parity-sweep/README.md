# solx-parity-sweep

Integration test that asserts EDR renders **the same Solidity stack trace** for a contract built with `solx` as for the same contract built with `solc` — across the revert/panic scenarios in `contracts/Scenarios.t.sol`.

## What it does

`test/sweep.ts` runs `hardhat test` twice (once with the `default` build profile = solc, once with the `solx` profile), parses the failing-test trace blocks from each run, and asserts per scenario that:

1. `Error:` reasons match.
2. Frame counts match.
3. Each frame's `Contract.function` location and `file:line` match.

## Pinned divergences

A small set of scenarios diverge from solc today and are pinned to their current solx output via `scenariosDivergingFromSolc` in `test/sweep.ts`. A golden mismatch means solx changed: either remove the entry (improvement) or update the pinned shape (regression). The pin set tracks the solx version selected by `hardhat-solx`'s version map, currently solx 0.1.6.

| Scenario | Why it diverges |
| --- | --- |
| `InternalRecurseTest` | solx's optimizer fully unrolls 3-deep self-recursion; inlined frames collapse. |

Previously pinned but since reached full parity, so they run under the strict parity check: `MutualRecursionTest` and `NestedModifierRevertTest` (debug info emitted by the merged `hardhat-solx` plugin), `InlineAssemblyRevertTest` and `InvalidOpcodeTest` (solx 0.1.6 maps assembly opcodes to statement lines, [solx#583](https://github.com/NomicFoundation/solx/pull/583)), and `BareModifierRevertTest` (bare-revert attribution recovered in [#1552](https://github.com/NomicFoundation/edr/pull/1552)).

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

Scenarios live in `contracts/Scenarios.t.sol`, committed in this project. To add one:

1. Add a target contract plus a failing forge test. Pinned divergence entries in `test/sweep.ts` reference line numbers, so appending at the end avoids re-pinning them.
2. Run `pnpm test`. Scenario keys are discovered dynamically from the failing-test output, so the new scenario joins the strict parity check with no further wiring.
3. If solx matches solc: done. If it diverges: pin the solx output in `scenariosDivergingFromSolc` with a comment saying why it diverges and which direction a future golden break means (improvement → remove or shrink the entry; regression → investigate).

This corpus is compiled live on every run and exists only for the sweep. The Rust tests in `edr_solidity`/`edr_provider` pin the same shapes against a separate committed fixture (`crates/edr_solidity/fixtures/`, regenerated with `gen-solx-fixtures` — see the [fixtures index](../../../crates/edr_solidity/fixtures/README.md)), so provider-path coverage for a new shape needs a scenario there too.

Note on toolchains: what the solx profile compiles with depends on hardhat-solx's settings (e.g. its explicit `-O1` optimizer default) and the solx release its version map selects (`SOLIDITY_TO_SOLX_VERSION_MAP`; `0.8.34` → `0.1.6` today). A locally linked hardhat-solx build can silently bring a different map — if pins break unexpectedly, check which solx actually ran via the bytecode's trailing CBOR `solcx` stamp.
