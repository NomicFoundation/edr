# revm-compat spike

Phase 0 spike for the revm@41 upgrade plan (see `docs/revm-41-op-compat-plan.md`):
prove that `op-revm` (which speaks revm@38) can execute over a revm@41 `Database`
through a type-conversion bridge without altering EVM semantics.

**Throwaway code.** This crate is deliberately outside the EDR workspace and is
never merged; the validated pieces get copied into `edr_op::revm_compat` during
Phase 2 of the plan.

## Result: GO ✅

**Database bridge:** the differential test passes — op-revm@20 (revm@38)
executing over a revm@41 database through `DbBridge` produces bit-identical
results to native execution. Three transactions (ETH transfer, contract call
exercising SSTORE/SLOAD on warm and cold slots, OP deposit with mint) run over
both a native revm@38 `CacheDB` and a revm@41 `CacheDB` wrapped in `DbBridge`.
Execution results, produced state diffs, and final post-state (balances,
nonces, code, storage) all match exactly, and the outbound 38→41 state
conversion commits correctly into the new-side database.

**Inspector bridge:** a revm@41 `Inspector` driven by the revm@38 (op-revm)
execution through `InspectorBridge` observes an *identical* trace to a native
revm@38 inspector — per-step pc, opcode, gas remaining, full stack contents,
and memory size; logs; `call`/`call_end` events (including the top-level
call); and frame initializations. Inspector *mutations* propagate too: a
revm@41 inspector charging extra gas on the interpreter mirror produces
exactly the same execution result as its native revm@38 counterpart. The
mechanism: the bridge materializes a revm@41 mirror of the interpreter per
callback and syncs mutations (gas, stack, memory, pc) back.

## Running

```bash
cargo test --test differential
```

(from this directory; the crate has its own `target/`, separate from the
workspace's)

## Layout

| Path | Contents |
|---|---|
| `src/convert.rs` | Value conversions, both directions (`CfgEnv`, `BlockEnv`, `TxEnv`, `AccountInfo`, `Bytecode`, `Account`/state, `ExecutionResult`, `HaltReason`, `SpecId`) |
| `src/db_bridge.rs` | `DbBridge`: revm@41 `Database` exposed as a revm@38 `Database` — the go/no-go piece |
| `src/inspector_bridge.rs` | `InspectorBridge`: revm@41 `Inspector` driven by revm@38 execution, via a per-callback interpreter mirror with mutation write-back |
| `src/hardfork.rs` | `OpHardfork` newtype (see finding 1) |
| `tests/differential.rs` | Native vs. bridged execution differential test |
| `tests/inspector_differential.rs` | Native vs. bridged inspector-trace differential test + mutation propagation test |

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

## Inspector bridge: scope and limitations

The spike proves **interpreter-level** inspection (steps, logs, calls,
creates, selfdestructs) with mutation write-back. What it deliberately does
NOT cover, and what that means for production:

- **Context-reading inspectors.** The inner inspector receives `()` as its
  context. Inspectors that read the journal or context (some `TracingInspector`
  configs, cheatcodes) would need a mirrored revm@41 `Context` over the live
  revm@38 journal — assessed as impractical. Production alternative: for
  context-dependent inspection on the OP path, instantiate *38-native*
  inspectors (e.g. via a second, 38-era `revm-inspectors` dependency, mirroring
  the dual-revm pattern) and convert their *output* (traces are value types)
  instead of bridging the inspector itself.
- **Inspector-forced halts** (`Interpreter::halt`) are not forwarded by the
  sync-back. Add control-flow write-back if production needs it (cheatcodes).
- **Cost:** the mirror is cached on the bridge, keyed by bytecode identity —
  the O(code) bytecode re-analysis and buffer allocations happen only on frame
  switches; steps within a frame refresh stack/memory contents in place. The
  per-step content copies are inherent to mirroring and are the same cost
  class as opcode-level tracing itself. Inspection with no step hooks
  (call-level tracers, gas reports) pays almost nothing.

Additional drift found while building it: `Gas.state_gas_spent` changed
`u64` → `i64` (signed for the EIP-7702 refund netting); `CallInputs` and
`CallOutcome` gained `charged_new_account_state_gas` (assert-false on the
revm@38 side); `SpecId` dropped six EVM-equivalent fork variants (folded in
conversion); `InstructionResult` gained internal `Suspend` (panic on
new→old); `CreateInputs` has private cache fields (converted via
accessors/constructor).
