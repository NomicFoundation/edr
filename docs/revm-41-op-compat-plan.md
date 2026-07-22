# Upgrading EDR to revm@41 with an op-revm compatibility layer

**Status:** Proposal / plan
**Scope:** Keep EDR core current with upstream revm and unblock Amsterdam-hardfork EIP
support on L1, while keeping OP-stack chains on a released `op-revm`.

---

## 1. Motivation

EDR needs to keep pace with upstream revm releases. Staying on an old revm blocks
adoption of newer EVM behavior — in particular, **full support for the Amsterdam hardfork
EIPs depends on moving to a v112-era revm** (crate `revm` 40/41). EDR core is currently on
`revm` 38.0.0, so revm upgrades are a recurring, expected maintenance activity, and the
next one is on the critical path for Amsterdam.

The blocker is the **decoupling of `op-revm` from the bluealloy/revm monorepo**.
`op-revm` is now released on its own cadence and lags revm:

- Latest released `op-revm` is **20.0.0**, which still declares `revm = "^38.0.0"`
  (`>=38, <39`). It does not accept revm 39/40/41.
- The revm work for the Amsterdam-era EIPs landed in a revm that maps to crate **40/41**.
- Therefore **no released `op-revm` overlaps the revm version we need.**

This is not a one-off gap. Because the two projects now release independently, revm ↔
op-revm version skew is expected to **recur** on every future revm bump. Any plan has
to assume divergence is the steady state, not a transient.

### Options considered

| Option | Summary | Verdict |
|---|---|---|
| **Wait** for an `op-revm` release on revm 40/41 | Blocks the L1 revm upgrade on an unbounded ETA | Rejected — L1 is the priority and can't wait |
| **Git-SHA pin** `op-revm` to a commit tracking revm 40/41 | Fast, config-only | Rejected — runs an **unreleased** EVM (bug fixes still landing on devnets); compromises OP EVM semantics to buy OP features we don't currently need |
| **Type-conversion (compat) layer** | Keep OP on released `op-revm` 20 / revm@38; move only L1 to released revm@41; convert at the op-revm seam | **Chosen** — see §2 |

### Why the compat layer fits our constraints

- **We prioritize L1 over OP, and OP already trails L1 in features.** OP not getting the
  newest Amsterdam EIPs immediately is the status quo, made explicit — not a regression.
- **It protects EVM semantics on both sides.** L1 runs the latest *released* revm; OP
  runs the latest *released* op-revm. Neither runs an unreleased EVM. The only thing OP
  gives up is feature *currency*, not *correctness*.
- **It quarantines the divergence into one crate** (`edr_op`) instead of forcing a
  workspace-wide abstraction rewrite now.

---

## 2. The compat alternative

### 2.1 Why two revm versions can coexist

EDR's multi-chain architecture is built on the `edr_chain_spec` traits (`ChainSpec`,
`EvmChainSpec`, `BlockEnvChainSpec`, …), which are parameterized over revm types. When
EDR core moves to revm@41, those **trait signatures become revm@41-typed**. `OpChainSpec`
implements the same traits, so its methods receive revm@41 types from the generic caller
(`crates/evm/src/lib.rs` calls `EvmChainSpecT::dry_run(...)` generically).

Rust treats a type from `revm-context@16` (the 38-era version) as a completely different
type from `revm-context@<41-era>`, even if structurally identical. So `edr_op` — which
must hand data to `op-revm` (revm@38) — needs to **convert revm@41 ↔ revm@38** internally.

### 2.2 Where the conversion lives

The conversion is **entirely internal to `edr_op`**. The rest of EDR is unaware: it passes
revm@41 through the generic trait exactly as it does for L1. `edr_op` cannot change its
*outward* interface (the shared trait) without changing the shared trait surface — and that
change *is* the larger "EDR owns its own types" refactor, which we are explicitly deferring.

```
generic EDR (revm@41)
      │  EvmChainSpec::dry_run           ← shared trait, revm@41-typed, UNCHANGED
      ▼
OpChainSpec::dry_run                     ← edr_op
      │  revm_compat: 41 → 38            (cfg, block, tx, DbBridge)
      ▼
OpEvm(Evm::new(...))                     ← op-revm / revm@38, runs
      │  revm_compat: 38 → 41            (ResultAndState, EVMError, state diff)
      ▼
returns revm@41 to generic EDR
```

The seam is narrow — only these entry points construct/run the op-revm EVM:

- `OpChainSpec::dry_run` (`crates/edr_op/src/spec.rs`)
- `OpChainSpec::dry_run_with_inspector` (`crates/edr_op/src/spec.rs`)
- `L1BlockInfo::try_fetch` (`crates/edr_op/src/spec.rs`, `crates/edr_op/src/block/builder.rs`)
- the OP block builder, which otherwise delegates execution to `EthBlockBuilder`

> **Note on custom structs.** For a *two-version* bridge, use **direct** `From<revm41::X>
> for revm38::X` conversions — not a neutral intermediate representation. A neutral IR
> (`revm41 → custom → revm38`) is double the conversions with no payoff until there are N
> backends behind one interface, which is the deferred "EDR owns its types" end-state.

### 2.3 Invasiveness map

Two facts, verified against crate manifests, keep most of the seam mechanical:

- `revm-primitives` **23.0.0 and 41.0.0 both require `alloy-primitives ^1.5.2`** → the leaf
  primitives (`Address`, `U256`, `B256`, `Bytes`, `TxKind`, eip-2930/7702 items) are the
  **same type** on both sides.
- `revm-precompile` **34.0.0 and 41.0.0 pin identical crypto ranges** (c-kzg, blst, k256,
  ark-bn254/bls12-381, secp256k1, modexp) → the heavy crypto is **shared**, single copy.

| Layer | Verdict | Notes |
|---|---|---|
| Hardfork / `OpSpecId` → `SpecId` | **Trivial** | One op-revm ⇒ one `OpSpecId`; add `From<OpSpecId>` for revm41 `SpecId` (enum map) |
| Value structs (`CfgEnv`, block env, `ExecutionResult`, `EVMError`, `HaltReason`) | **Low–Med, mechanical** | Fields are shared alloy / EDR-owned types → field-by-field `From` |
| `Transaction` trait (`revm_context_interface`) | **Medium** | Macro body uses only alloy + EDR's `ExecutableTransaction`; impl both revm-38 and revm-41 `Transaction` on `OpSignedTransaction` |
| `OpTxTr` (op-revm) | **Low** | Supertrait is revm-38 `Transaction` (covered above) |
| **`Database` trait** | **HIGH — real cost** | op-revm's `Journal`/`Evm` + `L1BlockInfo::try_fetch` need revm-38 `Database`; adapter converts `AccountInfo`/`Bytecode`/`AccountStatus` per call, on the hot path |
| **State-diff out** | **Med–High** | Committed `HashMap<Address, Account>` must convert revm-38 → revm-41; correctness-critical |

The value seams are cheap; the invasive, correctness-bearing work is the **`Database`
bridge** and the **committed-state-diff conversion**.

> **Struct-drift trap.** A new revm major can add fields to exactly the structs being
> hand-converted. On the shared `Account`/result structs, the bridge must **assert-fail,
> not silently default**, on any revm-41 field with no revm-38 representation — turning
> silent semantic drift into a loud boundary. (OP stays on revm-38 and simply won't have
> the newer behavior yet; that is expected.)

### 2.4 Impact on the rest of EDR

The **only architectural change** is the new `revm_compat` module inside `edr_op`.
Everywhere else, revm@41 is a **routine version bump** — the same shape of change as any
prior revm upgrade. The shared trait surface is unchanged in form.

Direct-revm dependents that migrate 38→41 as normal keep-up (plus vendored `crates/foundry/`):

```
edr_chain_l1, edr_chain_spec, edr_coverage, edr_gas_report, edr_provider,
edr_receipt, edr_runtime, edr_solidity, edr_transaction, precompile, primitives, tracing
```

---

## 3. Phased delivery

### Constraints that shape the phases

- The revm@41 bump is **atomic across the trait-coupled cluster** — you cannot migrate
  crate-by-crate, because the shared trait surface is defined against one revm version.
- **`edr_op` is a leaf**: only `edr_napi` depends on it, optionally (`op = ["dep:edr_op"]`,
  `op` is *not* a default feature). It can be excluded from the build without breaking any
  other crate. This is the compiling seam.
- There is a `/update-revm <tag>` skill that bumps the core revm crates and fixes
  mechanical breakage (including vendored Foundry, via the matching upstream Foundry PR).
  **Caveat:** the skill maps `crates/op → op-revm` from the bluealloy monorepo, which is
  stale post-decoupling. It must **not** touch `op-revm` — keep it pinned at 20.0.0.
  Peeling `edr_op` before running the skill ensures it never sees op.

### Git topology (squash-merged via merge queue)

```
main
 └─ epic/revm-41                 (integration branch; starts == main)
     ├─ PR1  → epic              L1 on revm@41, op peeled
     └─ PR2  → epic              op re-attached via revm_compat
 ... then  epic → main           (final full-CI validation)

 spike/revm-compat  → draft PR (evidence only, never merged)
```

Using an integration branch (not stacking PR1 directly onto main) is deliberate: Phase 1
*necessarily* produces a state where OP doesn't build. The integration branch keeps that
window off `main` entirely; `main` only ever sees the finished result.

---

### Phase 0 — Spike the bridge out-of-tree (go/no-go gate)

**Goal:** validate the only genuinely uncertain piece — does the `Database` + state-diff
bridge preserve EVM semantics — at the lowest cost, before any migration.

- Standalone throwaway crate depending on **both** `revm@41` and `op-revm@20` (Cargo rename).
- Implement `DbBridge` (revm@41 `Database` → revm@38) and the value/state-diff conversions.
- **Differential test:** run a transfer and a SLOAD/SSTORE tx through the bridge; assert the
  committed state diff matches native (a fixture, or op-revm run with native 38 types).
- Compiles independently. Open as a **draft PR** (`spike/revm-compat`) so reviewers can see
  the evidence separately from production wiring. Its validated code is copied into PR2; the
  draft PR is closed unmerged.

**Exit criterion:** the bridge round-trips a handful of representative txs correctly. If a
trait method proves un-bridgeable, we learn it here cheaply and can pivot to git-pin/wait.

> This is the good version of "start the compat work early." Do **not** build an in-tree
> identity (38→38) compat layer: it compiles but exercises none of the hard logic and gives
> false confidence.

---

### Phase 1 — Core bump to v112, op peeled off (PR1 → epic)

**Goal:** L1/generic workspace on revm@41, green and tested.

Do **all** dependency/manifest changes here so PR2 carries no dependency churn:

1. Remove `crates/edr_op` from `[workspace].members`. **Keep** `edr_napi`'s
   `edr_op = { optional = true }` line — this preserves the `op-revm` → revm@38 subtree in
   `Cargo.lock` (optional deps stay locked; features gate compilation, not resolution).
   **No lockfile churn.**
2. Run `/update-revm v112`, instructing it to leave `op-revm` at 20.0.0. It bumps the core
   revm crates and fixes the ~11 non-op crates + vendored `crates/foundry/`.
3. Pre-pin `edr_op`'s direct `revm-context` / `revm-context-interface` to the 38-era
   versions (`=16.0.1` / `=17.0.1`) so the lock reaches its final shape now. (edr_op is not
   compiled yet — this is manifest-only.)
4. Neutralize the ways `edr_op` gets pulled into a compile (all reverted in PR2):
   - `--workspace` — handled by step 1 (no longer a member; no `--exclude` needed).
   - `--all-features` commands (clippy, llvm-cov in `edr-ci.yml`) → replace with an explicit
     feature set omitting `op`.
   - `tracing` feature lists `"edr_op/tracing"` → drop that entry (the `--features tracing`
     doc test otherwise pulls edr_op).
   - napi build's explicit `--features op` → drop for Phase 1.

**Lock diff:** `+revm@41 tree` only (unavoidable); revm@38 + op-revm never leave.

**Green checkpoint:** L1 on revm@41, non-op tests pass. **Not shippable** (dropping OP would
regress users) — an internal integration checkpoint that unblocks the L1 upgrade.

**Reviewer note:** PR1 is large but **mechanical**. Attach the `/update-revm` Step-5 summary
(version deltas + why-each-change). Expect a big `Cargo.lock` diff = the revm@41 additions.

---

### Phase 2 — Re-attach op via compat (PR2 → epic)

**Goal:** full workspace + `op` feature compiles, op tests pass — shippable.

1. Re-add `crates/edr_op` to `[workspace].members`.
2. Revert the four feature/CI neutralizations from Phase 1 (restore `--all-features`,
   `"edr_op/tracing"`, napi `--features op`). Config returns to identical-to-main.
3. Add `edr_op::revm_compat` — the proven Phase-0 conversions (`From` impls, `DbBridge`,
   state-diff converter) plus `OpSpecId: Into<revm41::SpecId>`.
4. Wire the four seams through `revm_compat`: `dry_run`, `dry_run_with_inspector`,
   `L1BlockInfo::try_fetch`, the block builder.

**Lock diff:** ≈ **zero** (all versions already resolved in PR1; the 38-era crates edr_op
now points to are already present via op-revm).

**Green checkpoint:** full workspace + op feature green; op tests pass. Now shippable.

**Reviewer note:** PR2 is **small but subtle** — this is where scrutiny goes (the `Database`
bridge and state-diff conversion). Easy to review hard precisely because it isn't buried in
the revm bump.

---

### Final — `epic → main`

Squash-merge via the merge queue (team default). Full CI runs with op re-enabled to confirm
the whole thing works end-to-end. Because both PRs were reviewed into `epic`, this is
primarily a CI gate.

---

## 4. Operational notes

- **Drift.** An epic branch spanning a revm bump + vendored-Foundry changes conflicts easily
  with unrelated `main` churn. Merge `main → epic` on a regular cadence (and rebase open PRs).
  Timebox the epic.
- **Lockfile reading.** PR1's lock diff is large but is *only* the revm@41 additions — not a
  delete/re-add. PR2's should be ≈ empty. Flag this so reviewers don't over-read it.
- **`--locked` in CI.** Several CI commands use `--locked`. Ensure `Cargo.lock` is committed
  and consistent at each PR, or those steps fail.
- **This is a first approach.** The end-state (EDR defining its own neutral type surface, with
  revm/op-revm as swappable backends) is deliberately out of scope. If revm↔op-revm skew keeps
  forcing this seam open every release, revisit whether `edr_chain_spec` should stop naming
  concrete revm types in its trait signatures.

## 5. Checklist

- [ ] Phase 0: out-of-tree spike compiles; differential test passes; draft PR opened
- [ ] `epic/revm-41` branched from `main`
- [ ] PR1: `edr_op` peeled from members; `edr_napi` optional dep retained
- [ ] PR1: `/update-revm v112` run; op-revm left at 20.0.0
- [ ] PR1: `edr_op` revm deps pre-pinned to 38-era; feature/CI neutralizations applied
- [ ] PR1: green with op excluded; skill summary attached
- [ ] PR2: `edr_op` re-added to members; feature/CI neutralizations reverted
- [ ] PR2: `revm_compat` added; four seams wired; `OpSpecId → SpecId` conversion
- [ ] PR2: full workspace + op tests green
- [ ] `epic → main` squash-merged; full CI green
