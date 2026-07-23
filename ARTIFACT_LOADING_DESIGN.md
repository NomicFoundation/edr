# Design: Rust-side, lazy artifact & build-info loading (provider + Solidity test runner)

## Motivation

Hardhat currently owns all contract-artifact and build-info loading and pushes the data into EDR across NAPI. Two components consume it, in different ways:

- **Solidity test runner** — `runSolidityTests` accepts `artifacts: Array<Artifact>`. Hardhat reads every artifact JSON from disk, parses it in V8, and marshals the full payload (ABI + creation bytecode + deployed bytecode) across NAPI. For medium projects (~500 artifacts) that is ~30 MB of hex-encoded strings crossing NAPI synchronously before any test runs; for monorepos (forge-std + OpenZeppelin + project, 2–5k artifacts) it scales to hundreds of MB and a visible cold-start freeze. The vast majority of those artifacts are never deployed and never referenced by cheatcodes or traces.
- **JSON-RPC provider** — Hardhat passes only build-info file contents as buffers, but EDR **eagerly** parses every one of them at `ContractDecoder.withContracts`: full serde of compiler input + output, full source text retained in memory, every source map decoded into per-opcode `Instruction` tables up front. Build-info outputs are the largest files in a project (a single `.output.json` is easily 14+ MB), so this costs seconds of provider startup and a 10–20× avoidable steady-state memory footprint — even if no transaction ever fails and no stack trace is ever requested.

The proposal: move artifact/build-info discovery and loading into Rust behind a **shared per-project registry**, pass only directory paths across NAPI, eagerly load only cheap metadata, and lazily materialize bytecode, ABIs, and build-info-derived debug data for the contracts actually used.

## 1. Current state: how artifacts flow today

### 1.1 Provider flow

```
JS (HH3 network-manager.js)
  #getBuildInfosAndOutputsAsBuffers(): readBinaryFile of every
      artifacts/build-info/<id>.json + <id>.output.json          [raw bytes, never parsed in JS]
    → ContractDecoder.withContracts({ buildInfos, ignoreContracts })   [NAPI]
Rust
    → BuildInfoConfig::parse_from_buffers                        [EAGER, all build infos]
        → parse_{solc,solx}_compiler_metadata per build info:
            full serde of input+output JSON
            source model: SourceFile with full source text, AST walk,
              inheritance linearization, selector fixup
            decode ALL bytecodes (creation + runtime) and uncompress ALL
              source maps into Vec<Instruction> / pc_to_instruction
    → ContractDecoder { ContractsIdentifier(BytecodeTrie), RevertDecoder }
    → EdrContext.createProvider(..., contractDecoder)            [Arc<RwLock<…>> shared]
```

Key code:

- NAPI entry: `ContractDecoder::with_contracts` (`crates/edr_napi/src/contract_decoder.rs:27-39`); the decoder object is passed to `createProvider` and the inner `Arc<RwLock<edr_solidity::contract_decoder::ContractDecoder>>` is shared with the provider core (`crates/edr_provider/src/data.rs:307`), observability (`crates/edr_provider/src/observability.rs:78`), and the logger (`crates/edr_napi_core/src/logger.rs:109`).
- Eager parse: `BuildInfoConfig::parse_from_buffers` (`crates/edr_solidity/src/artifacts.rs:169`), per-build-info extraction in `crates/edr_solidity/src/artifacts/solc.rs` (`parse_solc_compiler_metadata:125`, `extract_solc_contract_metadata:175`, `decode_instructions:249`). The buffers cross NAPI zero-copy, but parsing is all-or-nothing at construction.
- What is retained for the whole session, per contract (`ContractMetadata`, `crates/edr_solidity/src/build_model.rs`): `pc_to_instruction: HashMap<u32, Instruction>` (usually larger than the raw bytecode), `normalized_code: Vec<u8>` for both bytecodes, and — shared per build info — `SourceFile`s owning the **full source text** of every file. The raw JSON (source maps, opcodes, AST) is decoded then dropped, so peak RSS during parse is higher still.

Runtime consumers:

- **Per-tx call labeling** (when `includeCallTraces` is enabled): `SolidityTracingInspector` calls `ContractDecoder::populate_call_trace_arena` on every collected trace (`crates/edr_solidity/src/tracing.rs:37,59`, wired at `crates/edr_provider/src/observability.rs:183-185`), which resolves contract/function **names** via the bytecode trie and ABI selectors. The logger then prints the pre-populated names (`crates/edr_napi_core/src/logger.rs:279,728`).
- **Failure stack traces**: `TransactionFailure::from_execution_result` → `get_stack_trace` → `try_to_decode_nested_trace_mut` under the decoder write lock (`crates/edr_provider/src/error.rs:659,684`; `crates/edr_solidity/src/contract_decoder.rs:435`). This is the only path that needs full `Instruction`/source-location metadata — and source _text_ is only read when rendering stack-trace frames (`error_inferrer.rs`, `trace_strategy.rs`).
- **Gas estimation** stack traces (`crates/edr_provider/src/data/gas.rs:79,212,297`).
- **Revert reasons / custom errors**: `RevertDecoder` inside the decoder, built from every contract's ABI `error` entries.

Mid-session updates: only `Provider.addCompilationResult(solcVersion, compilerInput, compilerOutput)` (`crates/edr_napi/src/provider.rs:65`) — the compiler input/output cross NAPI as fully-materialized JS objects (`serde_json::Value`), are parsed on a blocking thread, and appended to the shared decoder. Append-only: no eviction, and the trie lookup cache is cleared on each add. Separately, HH3's `network-manager.js` builds the `ContractDecoder` **once per NetworkManager** and reuses it for all providers — contracts compiled after the first provider creation are invisible to stack traces (documented limitation at `network-manager.js:170-197`).

### 1.2 Test-runner flow

```
JS (HH3 solidity-test/edr-artifacts.js)
  buildEdrArtifactsWithMetadata(): readArtifact(fqn) for EVERY artifact
      → JSON.parse in V8 → { abi: JSON.stringify(abi), bytecode, deployedBytecode,
                             linkReferences, deployedLinkReferences } [full strings]
  getBuildInfosAndOutputs(): build-info file pairs as Uint8Array      [raw bytes]
    → EdrContext.runSolidityTests(chainType, artifacts, testSuites,
                                  configArgs, tracingConfig, cb)      [NAPI]
Rust (crates/edr_napi/src/context.rs:199)
    → LinkingOutput::link: link ALL artifacts                         [EAGER, twice]
    → ContractsByArtifact (known_contracts: ABI + both bytecodes for every artifact)
    → RevertDecoder from ALL ABIs
    → TestContracts (test-suite ABI + bytecode CLONED out of known_contracts)
    → MultiContractRunner { known_contracts, test_contracts, libs_to_deploy,
                            revert_decoder, LazyContractDecoder(tracingConfig) }
```

Key code:

- NAPI types: `Artifact { id, contract }` with `ContractData { abi: String, bytecode: Option<String>, deployed_bytecode: Option<String>, link_references, … }` (`crates/edr_napi/src/solidity_tests/artifact.rs`) — everything crosses as strings; ABI is `serde_json::from_str`-parsed and bytecode hex-decoded at the boundary.
- Linking: `LinkingOutput::link` (`crates/edr_napi/src/solidity_tests.rs:26-56`) runs `Linker::link_with_nonce_or_address` over **all** artifacts and then `get_linked_artifacts` **re-links every contract a second time** before building `ContractsByArtifact` (`crates/foundry/evm/core/src/contracts.rs:20`).
- Test-suite extraction clones ABI + bytecode out of `known_contracts` into `TestContracts` (`crates/edr_napi/src/context.rs:286-289`) — duplicated in memory for the whole run (`crates/edr_solidity_tests/src/multi_runner.rs:102-104`).
- Build infos are wrapped in `LazyContractDecoder` (`crates/edr_napi/src/solidity_tests/runner.rs:17-57`): a `OnceLock` that parses **all** build infos on the first stack-trace decode. Lazy, but all-or-nothing — the first failing test with stack traces enabled pays the full multi-second parse.

Runtime consumers:

- **Deployment**: `libs_to_deploy` bytecodes, then each `TestContract` (`crates/edr_solidity_tests/src/runner.rs`).
- **Cheatcodes**: `vm.getCode` / `vm.deployCode` do a **linear scan** over all artifacts with name/path matching (`get_artifact_code`, `crates/foundry/cheatcodes/src/fs.rs:1059-1121`); `vm.etch` takes raw bytes and doesn't touch artifacts.
- **Test-function identification** (isolation/inspector): `find_by_deployed_code` — an **O(n) fuzzy scan** computing `bytecode_diff_score` against every artifact (`crates/foundry/evm/core/src/contracts.rs:79-87`, called from `crates/foundry/cheatcodes/src/inspector.rs:1213`).
- **Trace identification** (when traces are enabled): `LocalTraceIdentifier` over `known_contracts` — the only indexed lookup (sorted by code length + binary search), but rebuilt **per suite** (`crates/edr_solidity_tests/src/multi_runner.rs:396`).
- **Revert decoding**: the startup-built `RevertDecoder`; **gas reports** ride on the decoded traces.
- **Stack traces**: `LazyContractDecoder` (see above) — the same `edr_solidity::ContractDecoder` machinery as the provider, built from the same build-info buffers.

### 1.3 The JS side

Both flows sit on HH3's `ArtifactManagerImplementation`, which already maintains a lazily-built **path/FQN index** of the artifacts directory (`#fsData`) but caches no parsed content — every `runSolidityTests` invocation re-reads and re-parses every artifact JSON in V8. Build infos are read via `readBinaryFile` in both flows (duplicated code: `edr-artifacts.js getBuildInfosAndOutputs` vs a private `network-manager.js` method). The on-disk layout:

- `artifacts/<sourceName>/<Contract>.json` (`_format: "hh3-artifact-1"`): `contractName`, `sourceName`, `inputSourceName`, `abi`, `bytecode`, `deployedBytecode`, `linkReferences`, `deployedLinkReferences`, `immutableReferences`, `buildInfoId` (`solc-0_8_24-<40 hex>`). A few KB to ~120 KB each.
- `artifacts/build-info/<id>.json` (compiler input, ~hundreds of KB) + `<id>.output.json` (compiler output, easily 14+ MB).

### 1.4 Comparison

|  | Provider | Solidity test runner |
| --- | --- | --- |
| Crosses NAPI | Build-info buffers only (`TracingConfigWithBuffers`) | Full per-artifact ABI/bytecode strings **and** build-info buffers |
| Per-artifact JSON parsing | none (bytecodes come from build-info output) | every artifact, in V8, every run |
| Build-info parsing | **Eager**, all, at decoder construction | Lazy but **all-or-nothing** on first stack trace |
| In-memory structures | `ContractDecoder`: bytecode trie + `pc_to_instruction` + full source text + `RevertDecoder` | `ContractsByArtifact` + `TestContracts` (duplicate) + `RevertDecoder` + `LazyContractDecoder` (third copy of bytecode once parsed) |
| Hot-path lookups | Bytecode trie (indexed) + lookup cache | `find_by_deployed_code` / `getCode` linear scans; `LocalTraceIdentifier` indexed but rebuilt per suite |
| Mid-session updates | `addCompilationResult` (append-only); HH3 decoder otherwise stale across compiles | none (artifact set fixed per run) |
| Shared code | `TracingConfigWithBuffers` + `edr_solidity::ContractDecoder` type — but separate instances, opposite eagerness | same |

The two components never share loaded data today: a session that runs Solidity tests and then opens a provider parses the same build infos twice (once lazily, once eagerly) and materializes test-suite bytecode up to three times inside the test runner alone.

## 2. Target architecture: shared `ProjectRegistry`

```
                        ┌──────────────────────────────────────────────┐
   JS: paths only ────► │   ProjectRegistry (per (artifactsDir,        │
                        │                     buildInfoDir), shared)   │
                        │                                              │
                        │   ArtifactIndex           BuildInfoRegistry  │
                        │   (fs walk + headers,     (id → file paths,  │
                        │    fingerprints, lazy      per-id OnceLock   │
                        │    full bytecode)          lazy parse)       │
                        └───────┬──────────────────────────┬───────────┘
                                │                          │
             ┌──────────────────┴────────┐       ┌─────────┴───────────────────┐
             │ ArtifactBackend (trait)   │       │ Lazy ContractDecoder        │
             │ test runner: linker       │       │ (two-tier: trie of Indexed  │
             │ closure, fingerprint      │       │ entries → materialize per   │
             │ index, lazy bytecode      │       │ build info on stack trace)  │
             └──────────┬────────────────┘       └─────────┬───────────────────┘
              runSolidityTestsFromPath            provider + test-runner
                                                  stack-trace decoding
```

JS passes **directory paths**; Rust owns discovery, parsing, caching, and invalidation. A process-wide `Weak` map keyed by canonical `(artifactsDir, buildInfoDir)` lets a session running both Solidity tests and a provider discover and parse once.

### 2.1 The load-bearing fact

For HH3 artifacts, the artifact JSON's `bytecode`/`deployedBytecode` are **byte-identical** to the build-info output's `evm.*.object` (verified programmatically over the full integration-test artifact set, including linked contracts where both sides retain the `__$…$__` library placeholders). Artifacts also carry `linkReferences`, `deployedLinkReferences`, `immutableReferences`, `inputSourceName`, and `buildInfoId` — exactly the inputs the decoder uses to normalize bytecode for its trie (`get_library_address_positions` + `normalize_compiler_output_bytecode`, `crates/edr_solidity/src/library_utils.rs`, `crates/edr_solidity/src/compiler.rs`).

Therefore **contract identification and naming never require a build-info parse**. Only these do: AST-derived function model, source maps → `Instruction`s, `methodIdentifiers` selector fixup, and source text — i.e. exactly the data needed to _render a stack trace_, not to run transactions or tests.

### 2.2 Components and placement

- **`crates/artifact` (`edr_artifact`)** — pure types + the `ArtifactBackend` trait. Already a leaf dependency of `edr_solidity` and reachable from `foundry/*`, so cheatcodes and `foundry/evm/core` can depend on the trait without pulling in `edr_solidity`.

  ```rust
  pub struct ArtifactMeta {
      pub id: ArtifactId,                        // name, source (user sourceName), version
      pub input_source_name: Option<String>,     // key into build-info output.contracts
      pub build_info_id: Option<String>,
      pub abi: Box<serde_json::value::RawValue>, // lazily parsed → JsonAbi / errors
      pub kind: ArtifactKind,                    // Deployable | AbstractOrInterface
      pub deployed_fingerprint: Option<u64>,     // metadata-stripped hash
      pub path: PathBuf,
      pub mtime: SystemTime,
  }

  pub trait ArtifactBackend: Send + Sync {
      fn header(&self, id: &ArtifactId) -> Option<&ArtifactMeta>;
      fn full_contract(&self, id: &ArtifactId) -> Result<&FullContractData>;
      fn find_by_deployed_code(&self, code: &[u8]) -> Option<&ArtifactId>;
      fn iter_headers(&self) -> impl Iterator<Item = (&ArtifactId, &ArtifactMeta)>;
  }
  ```

- **`crates/edr_solidity/src/project/`** (new module — no new crate) — everything with I/O and build-model coupling:

  ```rust
  // registry.rs
  pub struct ProjectRegistry {
      artifacts: ArtifactIndex,
      build_infos: BuildInfoRegistry,
  }
  /// Process-wide cache; entries are Weak so registries free when unused.
  pub fn project_registry(artifacts_dir: &Path, build_info_dir: &Path)
      -> Result<Arc<ProjectRegistry>, RegistryError>;

  // build_info_registry.rs
  pub struct BuildInfoRegistry {
      paths: HashMap<String, BuildInfoPaths>,   // id → (input path, output path)
      parsed: DashMap<String, Arc<OnceLock<Result<ParsedBuildInfo, ContractDecoderError>>>>,
  }
  impl BuildInfoRegistry {
      /// Parses `<id>.json` + `<id>.output.json` on first call — one pair, not all.
      pub fn identified_contract(&self, id: &str, key: &ContractKey)
          -> Result<Option<IdentifiedContract>, ContractDecoderError>;
  }
  ```

  The only new parsing code is a "parse one build info from disk paths" wrapper over the existing `parse_split_solc_compiler_metadata` / `parse_solc_compiler_metadata` pipeline — the extraction logic is reused untouched.

- **Consumers**: the test runner's `FilesystemArtifactBackend` sits on `ArtifactIndex`; the provider's lazy `ContractDecoder` sits on `ArtifactIndex` (trie + ABIs) plus `BuildInfoRegistry` (full metadata). `VecArtifactBackend` wraps today's eager `Vec<Artifact>` as the fallback for programmatic artifacts.

### 2.3 `ArtifactIndex` load strategy

**Phase 1 — filesystem index (eager, parallel, ~ms).** Walk `artifactsDir` for `*.json`, build `BTreeMap<FQN, PathBuf>` + mtimes. No contents read.

**Phase 2 — header pass (eager, parallel).** Per artifact, parse only: ABI as `Box<RawValue>` (deferred `JsonAbi` conversion), a deployed-bytecode **fingerprint** (hex string with trailing solc CBOR metadata stripped, hashed — enabling `HashMap<fingerprint, ArtifactId>` lookup instead of linear scans), names, `inputSourceName`, `buildInfoId`, and link-reference _positions_. Result: `headers`, `by_fingerprint`, `paths` maps. Memory: raw ABIs + small structs only (~6 MB for a 2000-artifact monorepo vs ~120 MB today).

**Phase 3 — on-demand full load (lazy, cached).** `full_contract(id)` re-opens the file (page cache makes this near-free), parses just the bytecode fields, hex-decodes, and caches in `DashMap<ArtifactId, OnceLock<FullContractData>>`.

Per-artifact load classes:

| Class | Contents | When loaded |
| --- | --- | --- |
| **A** — test suites + transitive libraries | full (ABI parsed, both bytecodes, link refs) | eagerly at run start; count is small (<100 even for big projects) |
| **B** — contracts under test, mocks, deps, test bases | header + fingerprint eager; ABI/bytecode lazy | on first `vm.getCode`/`deployCode`, fingerprint hit, or revert-decode miss |
| **C** — interfaces / abstract contracts (`bytecode == "0x"`) | index entry only | ABI parsed only if actually referenced; often never |
| **D** — non-Hardhat-format (Vyper, plugins) | via `VecArtifactBackend` | caller mixes `artifactsDir` + a small `Vec<Artifact>` |

The linker's dependency DFS reads `link_references` only for artifacts it visits — the **transitive closure of the test suites**, not the whole project (often <10 % of artifacts in dependency-heavy repos). After the DFS, only closure members get `full_contract` calls.

## 3. Provider-side lazy `ContractDecoder`

The decoder keeps its public API and its `Arc<RwLock<…>>` embedding everywhere (`edr_provider`, `edr_napi_core::logger`, `edr_napi::Provider` are untouched); only its internals change.

### 3.1 Two-tier trie entries

`BytecodeTrie<T>` is already generic (`crates/edr_solidity/src/bytecode_trie.rs:35`), so a lazy entry type slots in without touching trie logic:

```rust
pub enum ContractEntry {
    /// Fully parsed (eager buffer path, addCompilationResult).
    Materialized(IdentifiedContract),
    /// Header-derived; full metadata materialized on demand.
    Indexed(Arc<IndexedBytecode>),
}

pub struct IndexedBytecode {
    normalized_code: Bytes,               // decoded, placeholders zeroed
    is_deployment: bool,
    library_address_positions: Vec<u32>,  // from (deployed)linkReferences
    immutable_references: Vec<ImmutableReference>,
    names: ContractNames,                 // sourceName, inputSourceName, contract name
    build_info_id: Option<String>,
    materialized: OnceLock<Result<IdentifiedContract, ContractDecoderError>>,
}
```

The trie search (`crates/edr_solidity/src/contracts_identifier.rs`) reads `library_address_positions` / `immutable_references` / `is_deployment` from either variant through small accessors — no algorithmic change.

```rust
pub struct ContractDecoder {
    contracts_identifier: ContractsIdentifier,   // now over ContractEntry
    revert_decoder: LazyRevertDecoder,           // OnceLock, built from artifact ABIs
    build_infos: Option<Arc<BuildInfoRegistry>>, // None ⇒ fully eager (today's behavior)
    ignore_contracts: bool,
}

impl ContractDecoder {
    pub fn new(config: BuildInfoConfig) -> Self;  // unchanged (buffer path)
    pub fn from_registry(registry: Arc<ProjectRegistry>, ignore_contracts: bool) -> Self;
    pub fn add_contract_metadata(&mut self, c: IdentifiedContract);  // unchanged
}
```

### 3.2 Two lookup tiers

- **Tier 1 — names (per-tx path).** `populate_call_trace_arena` / `get_contract_and_function_names_for_call` answer from a trie hit's `ContractNames` + lazily parsed ABI (selector → signature) **without any build-info parse**. This matters because the decoder is exercised on every traced/logged transaction, not just failures.
- **Tier 2 — full metadata (stack traces).** `try_to_decode_nested_trace_mut` needs `IdentifiedContract` (instructions, source locations). On an `Indexed` hit, `BuildInfoRegistry::identified_contract(build_info_id, key)` parses **exactly that build-info pair**, fills the entry's `OnceLock`, and opportunistically fills sibling contracts of the same build info (the parse produces them all anyway).

Materialization granularity is **per build info**, because the parse unit is inherently the file pair: the AST walk, inheritance linearization, and selector fixup are cross-contract within one compilation. A 14 MB output parse (~0.3–1 s) is paid once, on the first stack trace touching that build info, instead of the sum over all build infos at startup. A finer per-contract deferral of `decode_instructions` (keep the raw source-map string, `OnceLock<Vec<Instruction>>` in `ContractMetadata`) is a possible follow-up but ranks low: JSON parse + AST walk dominate.

Why not defer the _whole_ decoder (today's `LazyContractDecoder` model)? It dumps the full multi-second parse on the first failing transaction — the worst possible placement, exactly when the user is debugging — and doesn't help the per-tx naming path at all. The header-index design bounds cold start to a parallel header scan (~100–300 ms for thousands of artifacts) and first-trace latency to one build info. Optional polish: a low-priority background prewarm that materializes build infos after startup.

### 3.3 Memory and locking

- Steady state: trie keys (decoded normalized bytecodes, ≈ project binary size — the design's floor) + headers/ABIs. Source text is retained only for build infos actually hit, and only they hold `Instruction` tables.
- Tier-2 materialization must happen **outside** the decoder's write lock: clone the `Arc<IndexedBytecode>`, drop the lock, parse, `OnceLock::set`, re-enter. Otherwise a 1 s parse blocks all concurrent requests. Design this in from the start.

### 3.4 Semantics deltas (all benign, to document)

- Contracts from unsupported solc versions get tier-1 names (an improvement — today they are skipped entirely) but fail tier-2 materialization → treated as unidentified for stack traces (matches today).
- `ignore_contracts` is applied at index-build time using the header's contract name.
- Artifacts missing `inputSourceName` fall back to `sourceName`; if the build-info output key still misses, materialization fails gracefully to unidentified.

## 4. Test-runner integration

The test runner adopts the `ArtifactBackend` trait and `FilesystemArtifactBackend` (§2.2–2.3) and additionally:

- **Stack traces unify with the provider.** `runSolidityTestsFromPath` receives `buildInfoDir`; `LazyContractDecoder` is replaced by the same `ContractDecoder::from_registry(...)`. The runner inherits per-build-info laziness (no more all-or-nothing parse on first failure), and a provider created in the same process reuses the registry's parse.
- **Identification unifies.** The fingerprint index that replaces `find_by_deployed_code`'s O(n) fuzzy scan and the provider's trie are built from the same `ArtifactIndex` headers; `FullContractData` uses `Bytes` so the backend cache and trie keys share buffers where no linking occurred.
- **Single, closure-scoped link.** Replace the `link_with_nonce_or_address` → `get_linked_artifacts` double link (`crates/edr_napi/src/solidity_tests.rs:38-48`): run the link scoped to the test-suite closure, then patch bytecodes only for closure members using the already-computed `libraries`.
- **Kill the `TestContracts` duplication.** Hold `Arc<ContractData>` (or `ArtifactId` + backend handle) instead of cloned ABI + bytecode (`crates/edr_napi/src/context.rs:286-289`, `crates/edr_solidity_tests/src/multi_runner.rs:102`).

Cheatcodes (`vm.getCode`/`deployCode`) filter over in-memory headers and call `full_contract` on the match; revert decoding parses ABIs from `RawValue` on first decode-miss; `LocalTraceIdentifier` builds from headers + lazy bytecode fetch on candidate confirmation.

## 5. NAPI API evolution and back-compat

```ts
// NEW — provider
export interface ProjectArtifactsConfig {
  artifactsDir: string;
  buildInfoDir?: string; // default `${artifactsDir}/build-info`
  ignoreContracts?: boolean;
}
export class ContractDecoder {
  constructor(); // unchanged
  static withContracts(config: TracingConfigWithBuffers): ContractDecoder; // unchanged (HH2, programmatic)
  static fromProject(config: ProjectArtifactsConfig): ContractDecoder; // NEW
  /** Mtime-checked rescan; picks up newly compiled contracts. */
  refresh(): Promise<void>; // NEW, later step
}

// NEW — test runner
runSolidityTestsFromPath({
  chainType,
  artifactsDir,
  buildInfoDir,
  testSuites, // ["contracts/Foo.t.sol:FooTest", ...]
  configArgs,
  extraArtifacts, // optional Array<Artifact> — class-D fallback
  onTestSuiteCompletedCallback,
});
```

- `EdrContext.createProvider(..., contractDecoder)` is unchanged — the laziness lives inside the decoder object, so provider plumbing is untouched.
- **HH2 stays on the old paths verbatim**: combined build-info buffers via `withContracts`, full-artifact `runSolidityTests`. HH2 artifacts have no `buildInfoId` and use `.dbg.json` indirection, so they are intentionally out of scope for the path-based backend.
- **`addCompilationResult` keeps working**: it appends `Materialized` entries into the same trie — eager and lazy entries coexist by construction of `ContractEntry`.
- **`refresh()` fixes HH3's stale-decoder limitation**: Hardhat can call it after builds (or at each provider creation — the mtime-checked rescan is ~ms when nothing changed). New artifacts on disk become new `Indexed` entries; the trie is rebuilt from headers (cheap) while parsed build infos and unchanged headers are reused.
- **Build coordination**: Hardhat still owns `solc`; Rust trusts the directory is current at call time (already true via the build task). No mid-run rebuilds — rescans happen only at explicit `refresh()`/creation points.

## 6. Optimization inventory

| # | Optimization | Impact | Complexity |
| --- | --- | --- | --- |
| 1 | Identification trie built from artifact JSONs, not build-info parse | **Very high** — removes all build-info parsing from provider startup and the per-tx path; startup seconds → ~100–300 ms; steady-state memory ~10–20× down | M |
| 2 | Per-build-info lazy parse (`BuildInfoRegistry`, `OnceLock` per id) | **Very high** — first-stack-trace cost bounded to one build info; also fixes the test runner's all-or-nothing `LazyContractDecoder` | M |
| 3 | Tier-1 name labeling from headers | **High** — happy-path txs never parse build infos | S (falls out of #1) |
| 4 | Lazy `RevertDecoder` from artifact ABIs | Medium | S |
| 5 | Fingerprint index replacing `find_by_deployed_code` linear scan | High (test-runner hot loop) | S |
| 6 | Shared `ProjectRegistry` across provider + test runner | Medium-high for combined sessions | S–M |
| 7 | Closure-scoped single link (drop the re-link) | High for dependency-heavy monorepos | M |
| 8 | `test_contracts`/`known_contracts` dedup via `Arc<ContractData>` | Medium (memory) | M |
| 9 | `rayon` parallel header pass + parallel materialization | Medium (2–4× on remaining eager work) | S |
| 10 | Per-contract instruction/source-map decode deferral | Low-medium (parse dominates) | M |
| 11 | Source text on demand / `memmap2` for build-info files | Low once #2 is in; mmap shaves peak RSS during parse | S–M |
| 12 | Persistent `.edr-cache` index (headers, fingerprints, artifact→build-info map; mtime invalidation) | High for dev-loop cold starts on huge repos | M–L |
| 13 | `refresh()` / mtime rescan | Medium — correctness win (HH3 stale decoder) | S–M |

## 7. Implementation order

Shared pieces first; every step independently shippable. Provider track (P) and test-runner track (T) can proceed in parallel after the shared steps (S).

1. **T1 — `ArtifactBackend` trait** (in `edr_artifact`) + `VecArtifactBackend`; route the existing call sites (`crates/edr_napi/src/context.rs`, `crates/edr_solidity_tests/src/multi_runner.rs`, `crates/foundry/cheatcodes/src/{inspector,fs}.rs`, `crates/foundry/evm/core/src/contracts.rs`) through it. No behavior change. This is the painful, irreversible step — ship it alone.
2. **S1 — `ArtifactIndex`** (fs walk + header pass + fingerprints) in `edr_solidity/src/project/`, types in `edr_artifact`. Versioned serde for `hh3-artifact-1`; bail loudly on unknown `_format`.
3. **T2 — `FilesystemArtifactBackend` (eager full-load)** + `runSolidityTestsFromPath`. Hardhat switches; the JS read/parse/marshal cost disappears with no in-Rust behavior change.
4. **S2 — `BuildInfoRegistry`** (per-id lazy parse wrapper). Small, standalone.
5. **P1 — lazy `ContractDecoder`** (`ContractEntry`, `from_registry`, tier-1/tier-2) + `ContractDecoder.fromProject`. HH3 network-manager switches; HH2 keeps `withContracts`. The biggest user-visible provider win. Depends only on S1 + S2.
6. **T3 — lazy bytecode**: header/`full_contract` split live, fingerprint index replaces `find_by_deployed_code`, closure-scoped single link (folds in #7/#8).
7. **T4 — unification**: `runSolidityTestsFromPath` takes `buildInfoDir`; runner uses `ContractDecoder::from_registry`; global `Weak` registry cache lands.
8. **P2 — `refresh()`** / mtime rescan; decide `addCompilationResult` messaging.
9. **Measured follow-ups**: rayon (#9), `.edr-cache` (#12), instruction deferral (#10), mmap (#11).

Benchmark after each step on `js/benchmark` and a representative monorepo. Add a **differential test** asserting the artifact-header-built trie is key-identical to the build-info-built trie over the integration-test fixtures.

## 8. Risks and tradeoffs

- **Schema coupling / bytecode-equality assumption.** The design assumes artifact bytecode ≡ build-info output bytecode (verified for `hh3-artifact-1`). Guard: version-gated header structs; unknown `_format` or missing `buildInfoId` routes that artifact to the eager/buffer path.
- **Two normalization code paths.** Artifact-header normalization (from `deployedLinkReferences` positions) must byte-match `decode_evm_bytecode`'s output. Same inputs, same `normalize_compiler_output_bytecode` — enforced by the differential test.
- **First-stack-trace latency.** One build-info parse (~0.3–1 s for a 14 MB output) lands on the first failure. Acceptable versus seconds at startup; mitigable with background prewarm. Document it.
- **Lock-hold during materialization.** Must parse outside the decoder write lock (§3.3).
- **Trie memory floor.** Identification fundamentally requires all normalized bytecodes in memory. ≈ project binary size; shared with the backend cache via `Bytes` where unlinked.
- **Registry keying.** Canonicalize paths (Windows case/UNC); `Weak` entries so registries free when both consumers drop.
- **Error surfacing.** Today artifact problems fail at startup; lazily they fail mid-test/mid-request (e.g. a cheatcode referencing an artifact whose file vanished). Need a clear, attributable error path.

## 9. Open questions

- Persist the phase-2 header/fingerprint index across runs (`.edr-cache` under `artifactsDir`)? Probably yes for dev loops; invalidate on mtime. Follow-up step (#12).
- `simd-json`/`sonic-rs` for the header pass? Start with `serde_json` + `rayon`, measure, switch if needed.
- Bytecode cache LRU cap? Likely unnecessary — test runs are short and bounded; provider sessions are bounded by project size.
