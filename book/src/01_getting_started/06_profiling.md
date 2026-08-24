# Profiling

Three complementary profiling methods are available, differing in what they can see. Pick the lens that matches your question:

| Method | Sees | Blind to |
| --- | --- | --- |
| [Rust-only (`cargo flamegraph`)](#rust-only-profiling) | EDR's Rust internals, running a recorded scenario without Node.js | JavaScript, solc, anything outside the Rust binary |
| [Cross-language sampling (`perf`)](#cross-language-sampling-with-perf) | One profile spanning JavaScript ↔ napi ↔ Rust ↔ subprocesses (e.g. solc), all threads | Precise JS attribution (coarse function names, no inlining info); blocked time |
| [JS-level attribution (`--cpu-prof`)](#js-level-attribution-with---cpu-prof) | Per-function JavaScript/TypeScript time, inline-aware, GC/idle separated | Everything native: EDR's Rust threads and subprocesses appear as idle |

In practice the last two pair well: `perf` tells you how time splits between JavaScript, EDR and subprocesses; `--cpu-prof` tells you which JavaScript functions are responsible for the JavaScript share.

The [Hardhat repository](https://github.com/NomicFoundation/hardhat) ships a turnkey runner for both sampling methods over its end-to-end benchmark scenarios: `pnpm profiler` (see `scripts/README.md` there). The sections below document the underlying recipes so any Hardhat project can be profiled by hand.

## Rust-only profiling

For EDR-internal work, replay a recorded scenario under [`cargo flamegraph`](https://github.com/flamegraph-rs/flamegraph) — no Node.js involved, full Rust symbol fidelity:

```bash
cargo install flamegraph

CARGO_PROFILE_RELEASE_DEBUG=true cargo flamegraph \
  -o flamegraph_neptune.svg --root --release \
  -p edr_tool_cli -- scenario js/benchmark/scenarios/neptune-mutual-blue-protocol_8db6480.jsonl.gz
```

Recorded scenario files live in `js/benchmark/scenarios/`. See [Tools](../02_development/01_tools.md) for collecting new scenarios.

`--root` runs perf via sudo; on properly configured systems (see the [permissions appendix](#appendix-perf-permissions)) it can be dropped.

## Cross-language sampling with `perf`

Linux `perf` samples every process and thread, producing call stacks that span JavaScript, the napi boundary, EDR's Rust code and subprocesses in one profile. Two preparations make the stacks readable:

1. **EDR must be built with frame pointers and symbols.** The dedicated build profile does both (frame pointers let perf walk EDR's native frames and join them onto the JavaScript frames that called them; the `line-tables-only` debug info gives those frames Rust function names — expect a ~160 MB artifact instead of ~60 MB):

   ```bash
   cd crates/edr_napi
   pnpm build:perf-js
   ```

2. **The Hardhat project must load that build.** Either point the napi loader at the artifact directly — no reinstall needed:

   ```bash
   export NAPI_RS_NATIVE_LIBRARY_PATH=/path/to/edr/crates/edr_napi/edr.linux-x64-gnu.node
   ```

   or publish the build to a local registry and install it normally (see [Local EDR release](../02_development/03_local_release.md)).

Record with JavaScript symbolization enabled (V8 writes `/tmp/perf-<pid>.map` files that perf uses to name JIT frames):

```bash
NODE_OPTIONS="--perf-basic-prof --interpreted-frames-native-stack" \
  perf record -e cpu-clock -F 999 -g -- npx hardhat test solidity
```

- `-e cpu-clock` (software timer) is used instead of hardware `cycles` because virtualized environments such as WSL2 expose no PMU; at equal sampling rates the two attribute CPU time equivalently.
- `perf record` follows the whole process tree, so solc runs and worker processes are included automatically.
- Percentages from this method are shares of **CPU time**; time a thread spends blocked (I/O, waiting on a subprocess) is invisible.

Render the recording — post-process on the same machine, while the `/tmp/perf-<pid>.map` files still exist:

```bash
# Flamegraph (cargo install inferno):
perf script | inferno-collapse-perf | inferno-flamegraph > flamegraph.svg

# Interactive UI without extra tools: feed this file to https://speedscope.app
perf script > profile.linux-perf.txt

# Quick tables: CPU per binary, and per symbol
perf report --stdio --no-children --sort dso | head -20
perf report --stdio --no-children --sort dso,sym | head -40
```

## JS-level attribution with `--cpu-prof`

V8's sampling profiler attributes time to JavaScript functions precisely — inline-aware, with garbage collection and idle time separated out. It only sees JavaScript threads: while EDR computes on its Rust threads or solc runs, the JavaScript thread is counted as _idle_.

```bash
NODE_OPTIONS="--cpu-prof --cpu-prof-interval=1000 --cpu-prof-dir=$PWD/cpuprof" \
  npx hardhat test
```

Open the resulting `cpuprof/*.cpuprofile` files in [speedscope](https://speedscope.app) or Chrome DevTools (Performance → load profile).

Practical notes:

- `--cpu-prof-interval` is the sampling interval in microseconds: use ~100 µs for sub-second commands (density), ~1000 µs for long runs (file size).
- Passing the flags via `NODE_OPTIONS` makes child node processes (test workers, loaders) write their own profiles too.
- Profiles are written on **graceful exit only**. Processes that are killed — e.g. mocha parallel-mode workers, which the pool terminates — lose their profile. Disable parallel mode when you need those stacks.
- `bun` supports the same flags, but on its own command line rather than via `NODE_OPTIONS`: `bun --cpu-prof --cpu-prof-dir=... run test`.
- Two structural blind spots: native work appears as idle (combine with the `perf` method), and stacks resume from the event loop after every `await`, so a function's _caller_ is not always attributable.

## Event Tracing

To collect and visualise event traces (e.g. for understanding concurrency and latency rather than CPU time), build EDR with the `tracing` feature and render the resulting folded traces with [inferno](https://github.com/jonhoo/inferno):

```bash
cd crates/edr_napi
pnpm build:tracing
```

Running a Hardhat command with this build writes a `tracing.folded` file in the working directory. Convert it to a flamegraph (unordered, aggregated) or a flamechart (time-ordered):

```bash
cat tracing.folded | inferno-flamegraph > tracing-flamegraph.svg
cat tracing.folded | inferno-flamegraph --flamechart > tracing-flamechart.svg
```

## Appendix: perf permissions

`perf record` needs permission to open perf events. Symptoms of missing permission: `perf_event_open ... Operation not permitted` or perf's `perf_event_paranoid` advice.

**Bare Linux/WSL2 hosts** — sampling your own processes in user space works with `kernel.perf_event_paranoid` ≤ 2 (the common default). If your distribution sets a stricter value:

```bash
sudo sysctl -w kernel.perf_event_paranoid=2
```

**Containers (including devcontainers)** — Docker's default seccomp profile denies `perf_event_open` entirely, for any user. Two remedies, in order of preference:

1. _Custom seccomp profile, no added capabilities_ (recommended): take Docker's default profile and allow the one syscall. The kernel's `perf_event_paranoid` policy still applies inside, so the container can only sample its **own** processes in user space — nothing of the host. Append to the `syscalls` array of [Docker's default profile](https://github.com/moby/profiles/blob/f9bc03ec19b2dc4c091449b08e88f85c0caa9f0b/seccomp/default.json):

   ```json
   {
     "names": ["perf_event_open"],
     "action": "SCMP_ACT_ALLOW"
   }
   ```

   and start the container with `--security-opt seccomp=/path/to/profile.json`.

2. _`CAP_PERFMON`_ (fallback): add `"capAdd": ["PERFMON"]` to `devcontainer.json` (or `--cap-add PERFMON`). This also enables system-wide profiling of the shared kernel — broader than needed for profiling your own runs, but far narrower than `SYS_ADMIN`, which is root-equivalent and should not be used for this.

With either remedy, `perf` runs unprivileged — no `sudo` required. If you must run it via sudo (e.g. `cargo flamegraph --root`), use `sudo env PATH="$PATH"` so node and cargo remain resolvable.
