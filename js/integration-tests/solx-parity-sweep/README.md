# solx-parity-sweep

Integration test that asserts EDR renders **the same Solidity stack trace** for a contract built with `solx` as for the same contract built with `solc` — across the revert/panic scenarios in `contracts/Scenarios.t.sol`.

## What it does

`test/sweep.ts` runs `hardhat test` twice (once with the `default` build profile = solc, once with the `slang-solx` profile), parses the failing-test trace blocks from each run, and asserts per scenario that:

1. `Error:` reasons match.
2. Frame counts match.
3. Each frame's `Contract.function` location and `file:line` match.

## Pinned divergences

Scenarios that diverge from solc are pinned to solx's output via `scenariosDivergingFromSolc` in `test/sweep.ts`; every other scenario runs under the strict parity check. A golden mismatch means solx changed: either remove the entry (improvement) or update the pinned shape (regression). The pin set is specific to the solx release `hardhat-slang-solx`'s version map selects.

| Scenario | Why it diverges |
| --- | --- |
| `InternalRecurseTest` | solx's optimizer fully unrolls 3-deep self-recursion; inlined frames collapse. |

## Running

```sh
pnpm install
pnpm test
```

The `pretest` step builds the workspace's `@nomicfoundation/edr` napi binary so the sweep runs against current EDR sources. `@nomicfoundation/hardhat-slang-solx` is a regular dev dependency; on first use it downloads the solx release its version map selects into Hardhat's global compiler cache, so the first run needs network access. CI runs this package with the other `js/integration-tests/*` suites.

To try an unreleased plugin build, symlink it over the installed one (`ln -s <hardhat-clone>/packages/hardhat-slang-solx node_modules/@nomicfoundation/hardhat-slang-solx`); do not use `pnpm link`, which writes a machine-local `link:` dependency into the workspace manifests. A `pnpm install` restores the published package.

## Adding scenarios

Scenarios live in `contracts/Scenarios.t.sol`, committed in this project. To add one:

1. Add a target contract plus a failing forge test. Pinned divergence entries in `test/sweep.ts` reference line numbers, so appending at the end avoids re-pinning them.
2. Run `pnpm test`. Scenario keys are discovered dynamically from the failing-test output, so the new scenario joins the strict parity check with no further wiring.
3. If solx matches solc: done. If it diverges: pin the solx output in `scenariosDivergingFromSolc` with a comment saying why it diverges and which direction a future golden break means (improvement → remove or shrink the entry; regression → investigate).

This corpus is compiled live on every run and exists only for the sweep. The Rust tests in `edr_solidity`/`edr_provider` pin the same shapes against a separate committed fixture (`crates/edr_solidity/fixtures/`, regenerated with `gen-solx-fixtures` — see the [fixtures index](../../../crates/edr_solidity/fixtures/README.md)), so provider-path coverage for a new shape needs a scenario there too.

Note on toolchains: what the solx profile compiles with depends on hardhat-slang-solx's settings (e.g. its explicit `-O1` optimizer default) and the solx release its version map selects (`SOLIDITY_TO_SOLX_VERSION_MAP`, which maps `0.8.34` to `0.1.8`). A locally linked hardhat-slang-solx build can silently bring a different map — if pins break unexpectedly, check which solx actually ran via the bytecode's trailing CBOR `solcx` stamp.
