# Profiling `hardhat test solidity` — uniswap-v4-core (HH3 port)

- **Target:** `/workspaces/migrations/uniswap-v4-core` (Hardhat 3 port, `optimizer.runs = 44_444_444`, `viaIR: true`, cancun)
- **EDR:** local `main` (0.15.0) built with `build:perf-js` + `CARGO_PROFILE_NAPI_PUBLISH_DEBUG=1` (frame pointers + line tables)
- **Hardhat:** 3.4.5 from this monorepo (`overrides: hardhat>@nomicfoundation/edr: workspace:*`)
- **Host:** 12 cores, aarch64, Docker Desktop (linuxkit 6.10.14)
- **Sampling:** perf at 999 Hz per thread, no throttling or lost events
- **Suite:** 39 test suites, 598 tests, all passing

Harness, and the environment caveats that had to be worked around: [`README.md`](README.md).

Two fuzz regimes were measured, because they invert the answer:

- **`fuzz.runs = 1000`** — the project's actual config
- **`fuzz.runs = 10`** — matches `js/benchmark/patches/uniswap-v4-core.patch`, and makes the fixed per-invocation costs legible

## 1. Wall-clock phase breakdown (warm cache, unprofiled, 2 runs each)

Measured by `profile-uniswap.ts`, which mirrors Hardhat's real `solidity-test/task-action.ts` phase for phase.

| Phase | `runs=10` | share | `runs=1000` | share |
| --- | --- | --- | --- | --- |
| `hre:construct` | 16–18 ms | ~1% | 16 ms | 0.3% |
| `build:all` (solc, warm) | 204–277 ms | 12.6% | 208–235 ms | 3.8–4.3% |
| `artifacts:load` (139 artifacts) | 15–18 ms | 0.8% | 20–23 ms | 0.4% |
| `buildInfos:load` (61.8 MiB) | 44–191 ms | 2.7–8.8% | 52–110 ms | 1.0–2.0% |
| `inlineConfig:collect` | 223–409 ms | 13.7–18.8% | 226–254 ms | 4.2–4.7% |
| `runnerConfig:build` | 0.2–0.3 ms | ~0% | 0.2 ms | ~0% |
| `edr:context` | ~1 ms | ~0% | ~1 ms | ~0% |
| **`solidityTests:run`** (native) | **1112–1250 ms** | **57.6–68.6%** | **4775–4888 ms** | **88.0–90.2%** |
| **TOTAL** | **1622–2170 ms** |  | **5419–5423 ms** |  |

Cold compile, for reference: **190 s** (one-off; viaIR at 44.4M optimizer runs, solc runs serially at ~3.5–4.3 GB RSS per job).

## 2. CPU attribution (perf, 999 Hz)

Wall-clock hides that four things run concurrently. Sample counts by process/thread role:

| Component | `runs=10` |  | `runs=1000` |  |
| --- | --- | --- | --- | --- |
| EDR solidity-test runner (native, `tokio-rt-worker`) | 0.61 s | 14.3% | **18.85 s** | **83.9%** |
| Main process (JS: hardhat, artifact/build-info parsing, napi) | 1.58 s | 37.0% | 1.32 s | 5.9% |
| FFI subprocess — `node` | 1.22 s | 28.4% | 1.41 s | 6.3% |
| FFI subprocess — `npm` | 0.66 s | 15.5% | 0.67 s | 3.0% |
| node threadpool (`libuv-worker`, file I/O) | 0.20 s | 4.7% | 0.22 s | 1.0% |
| **Total CPU** | **4.29 s** |  | **22.47 s** |  |

## 3. Findings

### 3.1 Artifact loading is not the problem for this project

- JS side (`buildEdrArtifactsWithMetadata`, 139 artifacts): **15–28 ms**, <1% of wall clock.
- Native side (`LinkingOutput::link` → `Linker::link` → `get_linked_artifacts`, i.e. artifact deserialization + library linking): **11 samples ≈ 11 ms of CPU** at `runs=10` (0.26%), 6 samples at `runs=1000` (0.03%).
- `serde_json` anywhere in native code: 2 samples.

For uniswap-v4-core, lazy artifact loading would recover ~11 ms out of a 1.6–5.4 s run. The refactor cost described in that document is not justified by this project. It would need to be re-measured on a repo with an order of magnitude more artifacts before the `ContractsByArtifact`-as-a-trait work looks worthwhile.

### 3.2 The real artifact-side cost is build-info JSON, not artifacts

`buildInfos:load` reads **61.8 MiB** of build-info + output JSON (a single build info) purely so traces can be decoded. It costs 44–322 ms and it is the dominant JS-side work: the hottest main-process frames are `uv__fs_work` (12–14%), `Builtins_CreateTypedArray`, `v8::internal::Utf8DecoderBase::Decode`, `v8::internal::CopyChars`, `JsonParser::ScanJsonString`, plus `Scavenger` GC pressure from the resulting strings.

If anything on the artifact path deserves laziness, it is this — and unlike per-contract artifacts, it is already keyed per build info, so skipping it when traces are disabled is a much smaller change.

### 3.3 Inline-config collection is the largest fixed cost

`getTestFunctionOverrides` (Slang parsing for `forge-config:` comments) costs **223–409 ms** and returns **1 entry**. At `runs=10` that is 14–19% of the entire run — more than solc, more than artifact loading and build-info loading combined. It is a constant cost, so it is invisible at `runs=1000` (4–5%) but dominates short runs, which is exactly the edit-test loop case.

### 3.4 Cheatcode inspector hooks are ~24% of native CPU

Hottest leaf frames inside the native runner at `runs=1000` (18830 samples, 18826 with a resolvable frame). Each sample is attributed to its deepest resolved frame; percentages are of the EDR-native slice, reproducible with `analyze.py frames <stacks> --thread tokio-rt-worker`:

| Frame | samples | share of EDR native CPU |
| --- | --- | --- |
| `MainnetHandler::inspect_run` | 3642 | 19.35% |
| `Cheatcodes::step` | 2985 | 15.86% |
| `Cheatcodes::step_end` | 1379 | 7.32% |
| `keccak::backends::aarch64_sha3::p1600_armv8_sha3` | 970 | 5.15% |
| `revm_interpreter::instructions::stack::push` (2 monomorphisations) | 1017 | 5.40% |
| `mload` / `jumpi` / `mstore` / `jump` / `swap` | 1380 | 7.33% |

`Cheatcodes::step` + `step_end` = **23.18%** of native CPU (4364 samples, ±0.3 pp at 1σ). These are per-opcode inspector callbacks, paid on every instruction whether or not a cheatcode is in play. That is the single largest addressable native cost in this suite.

At `runs=10` the mix shifts toward fuzzing setup. Note these four are within each other's error bars (~±1.2 pp at 1σ on 612 resolved samples), so read them as comparable rather than ranked: `inspect_run` 9.80%, `FuzzDictionary::insert_push_bytes_values` 8.33%, `Cheatcodes::step` 8.17%, `IndexMap::insert_full` 7.84% — i.e. building the fuzz dictionary becomes as expensive as execution itself.

### 3.5 FFI costs ~2 CPU-seconds and is independent of fuzz runs

`test/utils/JavascriptFfi.sol` does:

```solidity
inputs[0] = "npm";
inputs[1] = "--silent";
inputs[2] = "--prefix";
inputs[3] = "./test/js-scripts";
inputs[4] = "run";
```

Every `vm.ffi` call spawns **`npm run`**, which then spawns node. Cost:

- `runs=10`: 1.88 CPU-s = **43.9%** of all CPU in the run
- `runs=1000`: 2.08 CPU-s = 9.3%

The near-identical absolute cost confirms these tests are pinned by inline config regardless of the global `fuzz.runs`. Roughly a third of it (0.66–0.67 s) is **npm's own startup**, doing no useful work. Invoking `node ./test/js-scripts/dist/<script>.js` directly instead of `npm run` would remove that outright. This is a property of the port, not of EDR — but it is large enough to distort any benchmark of this repo, and worth knowing about.

## 4. Recommendation ordering

1. `Cheatcodes::step`/`step_end` — 23.18% of native CPU, the only large addressable EDR cost.
2. `inlineConfig:collect` — 223–409 ms fixed, dominates short/iterative runs.
3. `buildInfos:load` — 61.8 MiB of JSON parsed per invocation; skip when traces are off.
4. Not artifact loading — measured at ~11 ms native, ~20 ms JS.

## 5. Reproducing

```bash
# 1. Build EDR with frame pointers + line tables
cd /workspaces/edr/crates/edr_napi
CARGO_PROFILE_NAPI_PUBLISH_DEBUG=1 pnpm run build:perf-js   # ~9.5 min

cd /workspaces/edr/js/benchmark/profiling

# 2. Phase timings only (no profiler) -- cheap and repeatable
node --import tsx profile-uniswap.ts --fuzz-runs 1000 --label warm
node --import tsx profile-uniswap.ts --fuzz-runs 10   --label warm

# 3. Under perf. record.sh handles all four environment caveats in section 6.
./record.sh 1000 /tmp/prof
./record.sh 10   /tmp/prof

# 4. Attribute
python3 analyze.py components /tmp/prof/stacks-r1000.out
python3 analyze.py frames     /tmp/prof/stacks-r1000.out --thread tokio-rt-worker
python3 analyze.py pattern    /tmp/prof/stacks-r1000.out

# 5. Render interactive flamegraphs, then view them in a browser
./render.sh /tmp/prof/stacks-r1000.out
./serve.sh                                  # http://localhost:8080
```

See [`README.md`](README.md) for the harness and the statistical caveats on reading sample counts.

Recorded phase timings and analysis output for the runs in this document are in [`results/`](results/). Raw captures are not: the `runs=1000` `perf script` dump is ~444 MB and is not portable across builds.

`render.sh` produces, per capture, `flamegraph-<label>.svg` and `flamegraph-<label>-edr-only.svg` (the latter filtered to the native runner threads, since the full graph is dominated by the FFI subprocesses and the JS main thread). They are interactive in a browser — click a frame to zoom, Ctrl-F to search — and are gitignored; regenerate rather than commit them.

Note there is no host `file://` path to them — `/workspaces` is a Docker named volume inside the VM, not a bind mount — so use `serve.sh` and let VS Code forward the port.

## 6. Environment caveats (not covered by `book/src/01_getting_started/06_profiling.md`)

Four things bite here, and all four silently produce wrong or empty data rather than failing loudly. Three are worked around by `record.sh`; the second is why `0x` was abandoned in favour of driving `perf` and `inferno` directly.

1. **`sudo` resets `PATH`.** `sudo perf record -- node …` fails with `Failed to collect 'task-clock' for the 'node' workload: No such file or directory`, which looks like a PMU problem but is just `node` not being found. Use `sudo env PATH="$PATH"`. (The doc mentions this for `0x`; it applies to bare `perf` too.)

2. **`0x --kernel-tracing` records the workload as root.** `platform/linux.js` spawns `sudo -E perf record … -- <node> …` with no way to drop privileges for the child, so it walks straight into caveat 4: 5 FFI tests fail and `solidityTests:run` short-circuits from ~1250 ms to 144 ms. Its sample rate is also hardcoded at `-F 99`, too coarse for a 2 s run. `0x --visualize-only` avoids the recording path and works fine, but adds a dependency the SVGs do not need, so `0x` is not used here. (An earlier draft of this document blamed a `sed -i` that `0x` runs over the binary `perf.data`. That was wrong — in sed BRE the pattern is an inert literal; measured, it removes 0 bytes. The real symbolization problem is caveat 3.)

3. **The `node` binary does not symbolize on this container.** Independently of 0x, files on certain overlay inodes — including `/usr/local/share/nvm/.../bin/node` — are recorded as `/ (deleted)`. Files under `/workspaces` (ext4) and `/tmp` resolve fine. Running a **copy** of the node binary from `/tmp` fixes it: unknown frames drop from **96% → 19%** and V8/node C++ symbols appear. A copy named `node` earlier in `PATH` does the same for the FFI subprocesses.

4. **`perf` must run as root, but the workload must not.** `/proc/sys` is read-only, so `perf_event_paranoid` cannot be lowered from 2. Running the workload as root instead makes Hardhat use `/root/.cache/hardhat-nodejs` (a 5.5 s build instead of 0.3 s), leaves a root-owned `cache/compile-cache.json` in the target repo, and fails 5 FFI tests — which short-circuits `solidityTests:run` to 144 ms and produces a profile of nothing. `setpriv --reuid=1000 --regid=1000 --clear-groups` plus an explicit `HOME` fixes it; perf still follows the child.

`perf` itself needs no `-e` flag: the default `cycles` event is unavailable on this kernel and perf silently falls back to `task-clock`.
