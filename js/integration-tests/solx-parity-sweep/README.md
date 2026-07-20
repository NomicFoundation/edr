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
| `MutualRecursionTest` | solx emits no subprogram for the test contract's dispatch PC; outer frame falls back to `internal@<pc>`. |
| `NestedModifierRevertTest` | solx flattens the modifier body into its function — 2 frames vs solc's 3. |

## Current state

Not yet running in CI. The suite has `@nomicfoundation/hardhat-solx` as an `optionalDependencies` entry because that package is not yet on the public npm registry; without it the suite self-skips.

## Prerequisites

To run the sweep, a local build of `hardhat-solx` must be linked into this package.

```sh
# 1. Clone the hardhat monorepo and check out the hardhat-solx branch.
git clone https://github.com/NomicFoundation/hardhat.git
cd hardhat

# 2. Install + build the monorepo so packages/hardhat-solx/dist exists.
pnpm install
pnpm --filter @nomicfoundation/hardhat-solx build

# 3. Link the built plugin into this package.
cd <edr-repo>/js/integration-tests/solx-parity-sweep
pnpm link <path-to-hardhat-clone>/packages/hardhat-solx
```

## Running

```sh
pnpm install
pnpm test
```

The `pretest` step builds the workspace's `@nomicfoundation/edr` napi binary so the sweep runs against current EDR sources. With no `hardhat-solx` linked the suite self-skips quickly.
