# revm-compat spike

Phase 0 spike for the revm@41 upgrade plan (see `docs/revm-41-op-compat-plan.md`):
prove that `op-revm` (which speaks revm@38) can execute over a revm@41 `Database`
through a type-conversion bridge without altering EVM semantics.

**Throwaway code.** This crate is deliberately outside the EDR workspace and is
never merged; the validated pieces get copied into `edr_op::revm_compat` during
Phase 2 of the plan.

## Result: GO ✅

The differential test passes: op-revm@20 (revm@38) executing over a revm@41
database through the bridge produces bit-identical results to native execution.

Three transactions — an ETH transfer, a contract call exercising SSTORE/SLOAD
(warm and cold slots), and an OP deposit with mint — run over both a native
revm@38 `CacheDB` and a revm@41 `CacheDB` wrapped in `DbBridge`. Execution
results, produced state diffs, and final post-state (balances, nonces, code,
storage) all match exactly, and the outbound 38→41 state conversion commits
correctly into the new-side database.

## Running

```bash
cargo test --test differential
```

(from this directory; the crate has its own `target/`, separate from the
workspace's)

## Layout

| Path | Contents |
|---|---|
| `src/convert.rs` | Value conversions, both directions (`CfgEnv`, `BlockEnv`, `TxEnv`, `AccountInfo`, `Bytecode`, `Account`/state, `ExecutionResult`, `HaltReason`) |
| `src/db_bridge.rs` | `DbBridge`: revm@41 `Database` exposed as a revm@38 `Database` — the go/no-go piece |
| `src/hardfork.rs` | `OpHardfork` newtype (see finding 1) |
| `tests/differential.rs` | Native vs. bridged differential test |

## Findings for the production `edr_op::revm_compat`

1. **`edr_op`'s `Hardfork` must become a newtype.** `HardforkChainSpec` requires
   `Into<EvmSpecId>` (revm@41's `SpecId` after the bump); op-revm only implements
   the conversion to revm@38's `SpecId`, and the orphan rule blocks EDR from
   adding it. `src/hardfork.rs` models the fix: a newtype wrapping `OpSpecId`
   that mirrors `into_eth_spec()`. This slightly widens PR2's footprint inside
   `edr_op` (the `Hardfork = op_revm::OpSpecId` alias changes type).
2. **Never copy the gas table across versions.** `CfgEnv`'s `[u64; 256]` table is
   indexed by `GasId`, whose indices can shift between revm majors — copying
   would silently misassign costs. The conversion re-derives the table from the
   spec via `new_with_spec` instead (safe as long as callers never customize gas
   tables, which EDR doesn't).
3. **The full-destructure guard works.** Every conversion destructures its input
   without a `..` rest pattern, so struct drift fails compilation instead of
   silently dropping data — it caught every real 38→41 difference (new Amsterdam
   cfg flags, `slot_num`, `Halt.logs`) while writing this spike. Exception:
   `CfgEnv` is `#[non_exhaustive]` on both sides, so that one conversion can't
   get the compile guard and needs manual review on every revm bump.
4. **Amsterdam cfg flags assert-false** (`enable_amsterdam_eip8037`,
   `amsterdam_eip7708_disabled`, `amsterdam_eip7708_delayed_burn_disabled`): they
   have no revm@38 representation, so the conversion refuses loudly rather than
   dropping them — the plan's "loud boundary" principle, applied.
5. **The drift is genuinely small.** `usize` → `AccountId`/`TransactionId`
   newtypes, `Account.original_info` and `ResultGas` went private
   (constructor-based conversion; note `new_with_state_gas(total, refunded,
   floor, state)` argument order), `gas_used()` deprecated in favor of
   `tx_gas_used()`. Everything else is field-identical, and the leaf primitives
   (shared alloy) plus crypto crates need no conversion at all.

## Not yet proven

The `Inspector` bridge for `dry_run_with_inspector` — EDR's tracing path. That
is the one remaining trait bridge this spike did not cover; it should be
validated before calling the whole seam de-risked.
