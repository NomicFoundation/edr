# Sketch: chain-owned inspection (alternative to the `InspectorBridge`)

**Status:** design sketch, follow-up candidate to `revm-41-op-compat-plan.md`
**Origin:** design discussion around the Phase 0 spike — "what if `EvmChainSpec`
declared the inspector as an associated type *without a bound*, and each chain
drove its own native inspector?"

## The idea

Today, inspection crosses the chain-spec boundary as **behavior**: callers hand
an arbitrary type implementing revm's `Inspector` trait into
`EvmChainSpec::dry_run_with_inspector`. That pins the shared trait surface to
one revm version's `Inspector` — which is why the OP path (op-revm, revm@38)
needs the `InspectorBridge` after the revm@41 upgrade.

The alternative: inspection crosses the boundary as **data**. The shared trait
takes a version-neutral *request* ("what to observe") and returns a
version-neutral *output* ("what was observed"). Each chain drives its own
**native** inspectors internally — L1 with revm@41 machinery, OP with revm@38
machinery. No bridge, no interpreter mirror, and OP gets full-fidelity
inspection (context and journal included).

Two Rust facts shape this design:

1. **No associated traits.** A chain cannot pick *which trait* bounds a generic
   parameter (`InspectorT: Self::InspectorTrait` is not expressible), so
   "OP declares revm@38's `Inspector` as its interface" cannot be written
   directly.
2. **Bounds are only needed by generic code that calls methods on the value.**
   The shared trait never drives the inspector — the chain impls do, and they
   know their concrete types. So the shared surface needs *no* revm inspector
   bound at all, provided construction and result-extraction also move behind
   the chain boundary.

Losing "callers can pass an arbitrary inspector type" is acceptable: EDR's
multichain design already routes all chain-varying behavior through
`ChainSpec`, and the provider only ever constructs a fixed, enumerable set of
observations (see inventory below).

## Trait shape

Prefer the form with **no exposed inspector type at all** — the associated
type turns out to be an implementation detail once construction and extraction
are chain-internal:

```rust
pub trait EvmChainSpec: ... {
    /// Runs a transaction while observing it as described by `inspection`.
    /// The chain drives its native inspectors internally.
    fn dry_run_with_inspection<BlockT: BlockEnvTrait, DatabaseT: Database>(
        block: BlockT,
        cfg: CfgEnv<Self::Hardfork>,
        transaction: Self::SignedTransaction,
        database: DatabaseT,
        custom_precompiles: &HashMap<Address, EdrPrecompileFn>, // neutralized
        inspection: InspectionRequest<'_>,
    ) -> Result<
        (ExecutionResultAndState<Self::HaltReason>, InspectionOutput),
        TransactionError<...>,
    >;
}
```

No `InspectorT` generic, no `Inspector` bound, no `PrecompileProviderT`
generic (custom precompiles pass as data, dissolving the
`type PrecompileProvider` GAT bound problem found alongside the spike).

## The request/output types mostly already exist

Grounded in what actually flows through the provider today
(`debug_trace.rs`, `data.rs`, `data/call.rs`, `observability.rs`):

```rust
pub struct InspectionRequest<'a> {
    /// debug_traceTransaction / debug_traceCall tracer.
    /// alloy-rpc-types-trace — already version-neutral.
    pub geth_tracing: Option<GethDebugTracingOptions>,
    /// Everything `EvmObserverConfig` holds today — already EDR-owned:
    pub call_override: Option<Arc<dyn SyncCallOverride>>,   // Fn(Address, Bytes) -> Option<CallOverrideResult> — neutral, even though mutating
    pub include_call_traces: IncludeTraces,
    pub on_collected_coverage: Option<Box<dyn SyncOnCollectedCoverageCallback>>,
    pub verbose_raw_tracing: bool,
    pub contract_decoder: Option<&'a ContractDecoder>,
}

pub struct InspectionOutput {
    /// alloy-rpc-types-trace — already neutral.
    pub geth_trace: Option<GethTrace>,
    /// The ONE entanglement: foundry_evm_traces::CallTraceArena is pinned to
    /// the workspace revm version. Needs an EDR-neutral arena (or a 38→41
    /// node-by-node conversion — trace nodes are addresses/bytes/kinds, i.e.
    /// value conversion).
    pub call_traces: Vec<CallTraceArena>,
}
```

Key observation: **`EvmObserverConfig` is already the neutral request** in all
but name, and the geth types in/out are already neutral. This design is less a
new abstraction than a promotion of what the provider already does.

## Call-site before/after (`debug_trace.rs`)

Before (today):

```rust
let mut debug_inspector = DebugInspector::new(tracing_options)?;   // revm-inspectors type
let observed = observe_execution(&observer_config, |observer| {
    dry_run_with_inspector::<ChainSpecT, _, _, _, _>(
        ..., &mut DualInspector::new(&mut debug_inspector, observer),
    )
})?;
let geth_trace = debug_inspector.get_result(..., &execution_result, &mut database)?;
```

After:

```rust
let (result, output) = dry_run_with_inspection::<ChainSpecT, _, _>(
    ...,
    InspectionRequest {
        geth_tracing: Some(tracing_options),
        ..InspectionRequest::from(&observer_config)
    },
)?;
let geth_trace = output.geth_trace.expect("requested");
```

`DebugInspector` construction, `DualInspector` composition, and `get_result`
extraction (which needs post-execution `ResultAndState` + database — both
version-specific) all move inside the chain impl, where the values are native.

## Entanglement inventory (the sizing answer)

| Piece | Status |
|---|---|
| `GethDebugTracingOptions` (request) | ✅ neutral (alloy-rpc-types-trace) |
| `GethTrace` (output) | ✅ neutral (alloy-rpc-types-trace) |
| `SyncCallOverride` (mutating hook) | ✅ neutral (`Fn(Address, Bytes) -> Option<CallOverrideResult>`) |
| Coverage callback | ✅ neutral (EDR-owned types) |
| `contract_decoder`, `IncludeTraces`, flags | ✅ neutral (EDR-owned) |
| `CallTraceArena` (output) | ✅ RESOLVED (2026-07-22): no neutral type needed. "Version-neutral" was over-strict — outputs only need to be *workspace-typed*, and generic code is on the workspace version. At refactor time the arena stays as-is (zero change, zero tail impact on the test-runner consumers: `edr_solidity_tests/src/result.rs`, `gas_report.rs`, `edr_gas_report`, `edr_provider`). At upgrade time only `edr_op` converts its internal old-generation arena old→new at its boundary: a tree walk with exactly 4 leaf maps — `InstructionResult` (map already written in the spike), `OpCode` (u8 wrapper, spec-identical bytes), `CallKind` + `StorageChangeReason` (revm-inspectors-local small enums). All other leaves are shared alloy/std; `decoded` fields are `None` at conversion time (decoding runs generic-side after crossing). Per-bump caveat: guard against arena shape drift between revm-inspectors generations (full-destructure discipline) |
| `PrecompileFn` (custom precompiles) | ⚠️ revm-typed fn signature — trivial EDR-neutral wrapper |
| Per-chain machinery (`DebugInspector`, observer internals) | 🔨 OP needs 38-era twins: a second `revm-inspectors` dependency (38-era), mirroring the dual-revm pattern |
| alloy-rpc-types-trace alignment | ⚠️ both revm-inspectors generations must accept the same alloy major; true today (2.x), can drift |

Touched call sites: `edr_chain_spec_evm` (trait), `crates/evm` (generic
wrappers), `edr_provider` (`debug_trace.rs`, `data.rs`, `data/call.rs`,
`observability.rs`), block builder's `add_transaction_with_inspector`.
The solidity-tests `EvmBuilderTrait` seam is NOT touched by this refactor —
see the dedicated section below.

## Precompiles: provider is machinery, custom fns are data (analyzed 2026-07-22)

Two version-pinning points on the shared surface today: the
`type PrecompileProvider<…>: PrecompileProvider<…>` GAT bound
(crates/chain/spec/evm/src/lib.rs:44) and the `PrecompileProviderT` generic on
`dry_run` — generic code builds
`OverriddenPrecompileProvider::with_precompiles(ChainSpecT::new_precompile_provider(spec), custom)`
(crates/evm) and passes it through.

The machinery/data split applies cleanly:

- The **provider** is execution machinery: `run(&mut self, ctx: &mut CTX,
  inputs: &CallInputs)` is invoked by the EVM mid-execution with the live
  context, plus stateful `set_spec`/`warm_addresses` (EIP-2929
  warming)/`contains`. It must not cross: `OverriddenPrecompileProvider`
  (EDR-owned, crates/precompile) moves behind the seam into the shared
  EVM-aware module; the GAT and method generic are deleted from the trait.
- The **custom precompile fns** are pure data: bare fn pointers
  `fn(&[u8], u64, u64) -> PrecompileResult` — bytes + gas + reservoir in,
  result out, NO context access. They cross as data.
- Output side is already neutral: `into_addresses()` →
  `HashSet<Address>` (consumed by the observer for trace filtering) joins the
  inspection output.

Key simplification: the napi surface is a **catalog, not an injection
point** — the only exported constructor is `precompileP256Verify()`
(wrapping revm's own Rust `P256VERIFY`); JS toggles catalog entries by
address, never defines logic.

Design choice for the data form: (a) EDR-neutral fn signature
(`fn(&[u8], u64, u64) -> EdrPrecompileResult` mirroring
Success/Revert/Halt) with per-chain neutral↔native adapters — keeps the Rust
extension point open; or (b) a catalog enum materialized per chain from its
own revm-precompile (both generations ship `secp256r1`) — zero conversion but
closes Rust extensibility. **Recommendation: (a)**; note the adapters are
neutral↔native per side — no cross-version conversion exists in this path,
so the previously flagged "precompile provider bridge" dissolves entirely.

Wrinkles: the third fn argument is the EIP-8037 reservoir (gas-model-coupled
— review per revm bump); `PrecompileStatus::Halt(PrecompileHalt)` needs a
neutral representation (small enum, spike-class).

## The Solidity test runner seam: resolved by substitution, not bridging

The test runner (vendored Foundry) does not go through `ChainSpec` — it has
its own seam: `EvmBuilderTrait` (defined in
`crates/foundry/evm/core/src/evm_context.rs`), through which each chain
supplies the EVM that Foundry's inspector stack (cheatcodes, tracers,
coverage) drives. Cheatcodes are exactly the kind of inspector that can never
be bridged — they read and mutate the EVM context by design — so at first
glance this seam looks like the hardest part of the whole plan.

Provenance context that explains the findings below: upstream forge was never
designed for multichain — EDR's fork retrofitted *type-level* genericity
(`ee8732903`, "make Evm generic in Foundry crates", #929) while the runner's
semantic model stayed single-EVM: the chain context is only ever constructed
as `Default` (the `ChainContextTr: Clone + Debug + Default` marker has no way
to carry real data) and transactions are built mainnet-shaped via setters. OP
support is type plumbing, not behavior — which is why substitution changes
nothing observable.

It turns out to need no bridging at all, for two reasons (analyzed
2026-07-22):

1. **The Foundry stack is already chain-generic.** `InspectorStack`,
   `Cheatcodes`, the backend, and the executor are all parameterized by chain
   type parameters (block, transaction, hardfork, chain context, EVM
   builder). Nothing about them is OP-specific; OP merely supplies op-revm's
   types as the type arguments (`OpEvmBuilder` in
   `crates/edr_op/src/solidity_tests.rs`). The cheatcode machinery itself
   never crosses a version boundary.
2. **op-revm's behavior is inert in tests.** What makes the OP EVM different
   from L1 — deposit transactions, L1 data fees, operator fees — never
   activates in Solidity tests: tests only construct plain calls/creates (no
   deposits anywhere in the path), and the chain context is
   `L1BlockInfo::default()`, i.e. all fee parameters are zero
   (`ChainContextTr` is just `Clone + Debug + Default`; the backend uses
   `::default()` — this holds in fork mode too). Only the `Base` halt/error
   variants are reachable. The op-specific behavior that *does* matter in
   tests is the precompile set (e.g. P256VERIFY from Fjord) and the hardfork
   mapping — both reconstructible natively on the new revm: the new
   `revm-precompile` ships `secp256r1`, op-revm's precompile module is a thin
   per-spec composition of it (~50 lines), and the hardfork mapping is the
   `OpHardfork` newtype from the spike.

So the treatment is **substitution**: at the upgrade, `OpEvmBuilder` stops
supplying op-revm types and instead supplies new-revm-native ones — the
mainnet EVM, an EDR-side OP precompile provider, the OP hardfork mapping, and
the plain transaction env. OP Solidity tests then don't depend on op-revm at
all, nothing crosses versions, and the cheatcode stack is untouched.

Caveat: this holds as long as OP Solidity tests don't need real fee or
deposit simulation — neither is supported today, so nothing regresses; if
that ever becomes a requirement, revisit (either extend the
inspection-as-data refactor to this entry point, or reassess).

## Implementation shape (agreed 2026-07-22)

- **Step 1 (single revm version):** the machinery (`EvmObserver` composition,
  `DebugInspector` construction, result extraction) moves out of the generic
  provider into one shared EVM-aware module. The collectors are already
  context-generic (`impl<ContextT> Inspector<ContextT>`), so one
  implementation serves all three chain types — chain-parameterized,
  revm-version-pinned. All three `ChainSpec` impls delegate to it; the trait
  (request in, output out) is the abstraction boundary and the sharing is
  invisible to callers.
- **Step 2 (upgrade):** L1 + generic keep the shared module (it moves to the
  new revm with the ordinary bump). OP stops delegating and instantiates its
  own machinery against its revm version: EDR-owned collectors get a second
  `Inspector` impl for the old trait (dual-impl, same pattern as the
  transaction traits); foreign pieces (`TracingInspector`/`DebugInspector`)
  come from an old-era `revm-inspectors` dependency inside `edr_op`; outputs
  (trace arena) convert old→new at OP's boundary. The asymmetry — two chains
  sharing one implementation, one diverging — is invisible through the trait.

## Call-site audit (2026-07-22)

Every production inspector, traced to its origin — the basis for the
feasibility claim:

| Call site | Inspector passed | Constructed from | Read back |
|---|---|---|---|
| `debug_trace.rs` (`debug_traceTransaction`) | `DualInspector(DebugInspector, EvmObserver)` | `GethDebugTracingOptions` + `EvmObserverConfig` | `GethTrace` + `CallTraceArena` |
| `data.rs:~1880` (`debug_traceCall`) | same | same | same |
| `data.rs:~2369` (`eth_call`) | `EvmObserver` | `EvmObserverConfig` | traces, console logs, coverage |
| `data/gas.rs` ×2 (`eth_estimateGas`) | `EvmObserver` | `EvmObserverConfig` | traces + result |
| miner → block builder → `chain_l1/block.rs:~470` | `EvmObserver` (reused per block, per-tx data extracted) | `EvmObserverConfig` | per-tx: call traces, executed-bytecode map, console logs, coverage |
| trait-default `dry_run` | `NoOpInspector` | — | — |
| `edr_coverage` e2e test | `CoverageCollector` | a callback | coverage hits |

`EvmObserver` is a closed composite of five parts: `ExecutedBytecodeCollector`,
`Option<CodeCoverageReporter>`, `ConsoleLogCollector`, `Mocker` (call
override), `SolidityTracingInspector`.

### Output sufficiency: does request → output cover every usage? (checked 2026-07-22)

Two concerns raised and resolved:

1. **Post-execution inspector usage.** `debug_inspector.get_result(...)` runs
   *after* execution. Its five arguments: `tx_context` (plain data, known
   before execution → request field), the transaction, the block env, the
   `ResultAndState`, and the database. The last three are **version-typed**
   values that exist inside `dry_run_with_inspection` at exactly the right
   moment — and that the caller could not hold post-split anyway. So
   extraction must (and can) move chain-side; the output carries the finished
   `GethTrace`. Same pattern for `observe_execution`'s post-processing
   (`collect_and_report(execution_result.precompile_addresses())`) and the
   miner's per-tx `flush_inspector_data` (→ per-tx outputs of
   `add_transaction_with_inspection`).
2. **Callers decide composition.** Today's entire production composition
   vocabulary is: `EvmObserver` (parts toggled by config) plus optionally a
   `DebugInspector`, glued by `DualInspector` (pure plumbing, not semantics).
   Expressible as request fields. **Design commitment:** a future inspection
   kind is no longer "write an inspector, pass it in" — it means extending
   the request type and every chain's implementation. Deliberate trade of
   flexibility for encapsulation; consistent with inspection being a
   chain-side capability.

Conclusion: **every production inspector is already constructed from exactly
two data sources** — `EvmObserverConfig` and (debug-trace paths only)
`GethDebugTracingOptions` + a small `TransactionContext` (block hash / tx
index / tx hash). The `InspectionRequest` is those two put together; the
refactor at each call site deletes construct-compose-pass ceremony
(`DebugInspector::new`, `observe_execution` closures, `DualInspector::new`)
and passes the configs directly. `DualInspector` disappears — composition
becomes chain-internal. The mining path additionally needs per-transaction
outputs from `add_transaction_with_inspection` (mechanical: the miner already
extracts per-tx `transaction_inspector_data` today).

## Consequences

- **Deletes `InspectorBridge`** (mirror, sync protocol, its limitations) and
  the **precompile-provider bridge** — both replaced by native per-chain
  execution.
- **OP gains full-fidelity inspection**: context/journal access and mutation
  work natively, so context-dependent inspection (cheatcodes on the OP path)
  becomes possible — the one thing the bridge can never provide.
- This is the inspection slice of the plan's §4 end-state ("EDR stops naming
  revm types in its trait signatures"), done first because inspection is where
  version pinning bites hardest.

## Sequencing recommendation: do it BEFORE the upgrade

Key realization: **this refactor does not depend on the revm@41 upgrade — and
it is dramatically cheaper without it.** Today the whole workspace is on one
revm version, so moving inspection behind `ChainSpec` is a pure architecture
refactor: no dual dependencies, no 38-era twins, no conversions. Every cost in
the entanglement inventory that mentions "per-chain twins" or "dual
revm-inspectors" exists only if the refactor happens *during* the version
split. Done first, on `main`, as ordinary single-version PRs:

1. **Pre-upgrade refactor (this document):** move inspector construction and
   result extraction behind `EvmChainSpec::dry_run_with_inspection`; neutral
   request/output types; custom precompiles as data. All chains use the same
   (current) revm internally. Include the **EDR-neutral call-trace output**
   now — it's the one type that re-entangles at upgrade time, and
   single-version is the cheap moment to neutralize it.
2. **PR1 (revm@41 bump)** gets easier: generic provider code no longer touches
   revm-inspector types, so there is less to migrate.
3. **PR2 (op re-attach) shrinks:** the `InspectorBridge` (interpreter mirror +
   sync protocol) and the precompile-provider bridge are **no longer needed**
   — OP simply keeps its internal inspection machinery on the old generation
   (a second, older `revm-inspectors` dependency inside `edr_op` only). What
   remains of PR2 is the part the spike proved cheap: `DbBridge` + the value
   conversions for the `dry_run` seam (cfg/block/tx/result), which still cross
   the trait.

Caveats: this touches the shared trait surface before the upgrade (that is
when touching it is cheapest and safest — the original plan's avoidance was
about not doing it *mid-split*), and it is a real review cycle that precedes
PR1 on the timeline, so it needs team buy-in.

The spike's `InspectorBridge` remains as evidence of what the bridge path
would cost if the team prefers upgrade-first sequencing after all; the
`DbBridge` and value conversions carry into PR2 either way.
