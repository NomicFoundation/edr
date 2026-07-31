# Solidity-test profiling harness

Tooling used to produce [`uniswap-v4-core-profiling.md`](uniswap-v4-core-profiling.md): a component-level breakdown of `hardhat test solidity` on a Hardhat 3 project, splitting artifact loading, inline-config collection, the native EDR test runner, and FFI subprocesses.

Everything here is a profiling tool, not part of the benchmark suite — `js/benchmark/tsconfig.json` only includes `src/**` and `test/**`, so nothing in this directory is compiled by `pnpm build`.

## Files

| File | Purpose |
| --- | --- |
| `profile-uniswap.ts` | Phase-instrumented driver. Mirrors Hardhat's real `solidity-test/task-action.ts` phase for phase and times each one. |
| `record.sh` | `perf record` wrapper. Encapsulates the four environment workarounds below. |
| `analyze.py` | Attributes samples to components; drills into a thread's hot frames; sizes specific code paths; verifies the sample rate. |
| `render.sh` | Renders a capture to an interactive HTML flamegraph (0x) and SVG flamegraphs (inferno). |
| `serve.sh` | Serves the rendered flamegraphs over HTTP, since there is no host `file://` path to them. |
| `patches/0x-kernel-tracing.patch` | Fix for a `0x` bug that corrupts its own `perf.data` (see below). Only needed if you record with `0x` instead of `record.sh`. |
| `results/` | Phase timings (JSON) and saved analysis output from the recorded runs. |

Raw captures and renders are **not** checked in: a `perf script` dump of the `runs=1000` run is ~444 MB, its folded form ~109 MB, and the rendered HTML ~23 MB. None of it is portable — symbolization needs the exact binaries. Regenerate with `record.sh` and `render.sh`.

## Prerequisites

```bash
# EDR with frame pointers + line tables. Without frame pointers, native stacks
# do not unwind; without line tables you lose file/line in the flamegraph.
cd ../../../crates/edr_napi
CARGO_PROFILE_NAPI_PUBLISH_DEBUG=1 pnpm run build:perf-js    # ~9.5 min (fat LTO)

sudo apt-get install -y linux-perf
cargo install inferno rustfilt
```

## Usage

Phase timings only, no profiler — this is the cheap, repeatable measurement:

```bash
cd js/benchmark/profiling
node --import tsx profile-uniswap.ts --fuzz-runs 1000 --label warm
node --import tsx profile-uniswap.ts --fuzz-runs 10   --label warm --json phases.json
```

Options: `--repo <path>` (default `/workspaces/migrations/uniswap-v4-core`), `--fuzz-runs <n>`, `--grep <substr>` to filter suites, `--json <path>`, `--label <name>`.

`fuzz.runs` is a driver parameter rather than a config edit, so both regimes can be profiled without touching the target repo. Profile both: they invert the answer. At `runs=1000` the native runner is ~88% of wall clock and fixed costs vanish; at `runs=10` the fixed per-invocation costs (inline config, build-info parsing) are what you actually see in an edit-test loop.

Under perf:

```bash
./record.sh 1000 /tmp/prof        # records, resolves symbols, prints attribution
./record.sh 10   /tmp/prof
```

Then drill in:

```bash
python3 analyze.py components /tmp/prof/stacks-r1000.out
python3 analyze.py frames     /tmp/prof/stacks-r1000.out --thread tokio-rt-worker --top 15
python3 analyze.py frames     /tmp/prof/stacks-r1000.out --thread node-prof
python3 analyze.py pattern    /tmp/prof/stacks-r1000.out
python3 analyze.py pattern    /tmp/prof/stacks-r1000.out --pat 13LinkingOutput
python3 analyze.py rate       /tmp/prof/stacks-r1000.out
```

`analyze.py components` groups by process/thread **role**, not by DSO, because that is what actually separates the components in this workload: EDR's native runner runs on `tokio-rt-worker` threads, Hardhat's JS on the main thread, artifact and build-info file I/O on `libuv-worker`, and `vm.ffi` shells out to entire `npm`/`node` subprocesses. A DSO-based split buries the FFI cost — which was 43.9% of all CPU at `runs=10`.

`analyze.py pattern` matches **mangled** substrings (Rust v0 embeds `<len><name>`, hence `13LinkingOutput`), because matching post-demangling would mean demangling every frame of every sample.

## Viewing flamegraphs in a browser

```bash
./render.sh /tmp/prof/stacks-r1000.out          # -> alongside this harness
./serve.sh                                      # http://localhost:8080
```

`render.sh` writes three things per capture:

| Output | Notes |
| --- | --- |
| `flamegraph-<label>.html` | 0x's interactive page. JS-aware: collapsible frames, a search box, and tier filters (optimized / not-optimized / inlined / C++ / regexp). Best for reading the JS side and navigating a large tree. ~14–23 MB. |
| `flamegraph-<label>.svg` | inferno. Also interactive in a browser — click a frame to zoom, Ctrl-F to search. Much smaller, and easy to post-process or diff. |
| `flamegraph-<label>-edr-only.svg` | Filtered to `tokio-rt-worker` frames. The full graph is dominated by the FFI subprocesses and the JS main thread, which buries the EDR internals. |

Two things worth knowing:

- **There is no host `file://` path.** In this devcontainer `/workspaces` is a Docker _named volume_ inside the Linux VM (`/dev/vda1[/docker/volumes/...]`, ext4), not a bind mount from the host, and `/tmp` is container-only. So opening the file directly from the host browser does not work — hence `serve.sh`, which serves the directory over HTTP for VS Code to forward. (VS Code's Simple Browser also works, but still needs the URL.)
- **`render.sh` drives 0x through `--visualize-only`**, rendering from a capture that `record.sh` already made. This is deliberate: 0x's own recording mode runs the workload as root, which trips caveats 2 and 3 below — 5 FFI tests fail and `solidityTests:run` short-circuits to 144 ms, i.e. a beautifully rendered picture of nothing. Rendering from an existing capture keeps 0x's viewer without its recording bugs.

0x discovers a capture by filename (`/^stacks\.(.*)\.out$/`), so a file named `stacks-r1000.out` is silently not found; `render.sh` stages a correctly-named copy. Large captures also need `NODE_OPTIONS=--max-old-space-size=12000`, which it sets.

## Reading the numbers

- Sample counts convert to CPU-seconds by dividing by the sample rate (999 Hz default), and are **per-thread**: 22445 samples over 5.78 s wall clock means ~3.9 cores busy, not 22 s elapsed.
- Treat counts as Poisson when judging whether a difference is real: σ = √n. At `runs=1000` the EDR-native share (18830 samples) is ±0.6 pp; at `runs=10` individual frames inside the native slice (612 resolved samples) carry ~±1.2 pp, so frames within a few points of each other are not separable. Add captures rather than raising `-F` to sharpen those.
- `analyze.py frames` attributes each sample to its deepest **resolved** frame. Attributing only samples whose literal leaf resolved instead shrinks the denominator and inflates every share by ~0.5 pp — the two are not interchangeable.
- `rate` is the sanity check that perf was not throttled: `samples/wall-sec ÷ requested Hz` should equal the average number of on-CPU threads. Cross-check with `sudo perf report -i <data> --stats | grep -iE 'THROTTLE|LOST'`.

## Environment caveats

Four things had to be worked around on this container (Docker Desktop, linuxkit 6.10.14, aarch64). All four produce wrong or empty data rather than failing loudly, and `record.sh` handles all of them. Listed here because they will bite anyone profiling without it.

1. **`sudo` resets `PATH`.** `sudo perf record -- node …` fails with `Failed to collect 'task-clock' for the 'node' workload: No such file or directory`, which reads like a missing PMU but is just `node` not being found. Use `sudo env PATH="$PATH"`. (`06_profiling.md` notes this for `0x`; it applies to bare `perf` too.)

2. **`sudo` sets `HOME=/root`.** Hardhat then uses `/root/.cache/hardhat-nodejs`, re-resolves compilers, and the warm build goes from ~0.3 s to 5.5 s — silently profiling a cold build. It also leaves a root-owned `cache/compile-cache.json` in the target repo, which breaks the next non-root run. Pass `HOME`/`USER`/`LOGNAME` explicitly.

3. **perf must be root, but the workload must not.** `/proc/sys` is read-only in the container, so `perf_event_paranoid` cannot be lowered from 2. Running the workload as root instead makes 5 FFI tests fail, which short-circuits `solidityTests:run` to 144 ms — a profile of nothing. `setpriv --reuid=<uid> --regid=<gid> --clear-groups` drops privileges for the child; perf still follows it.

4. **The `node` binary does not symbolize on overlayfs.** Files on certain overlay inodes — including `/usr/local/share/nvm/.../bin/node` — are recorded by the kernel as `/ (deleted)`, so ~96% of frames come out `[unknown]`. Files under `/workspaces` (ext4) and `/tmp` resolve fine, so running a **copy** of the binary from `/tmp` fixes it (96% → 19% unknown, and V8/node C++ symbols appear). A copy named `node` early on `PATH` does the same for the `npm`/`node` subprocesses that `vm.ffi` spawns.

Also: perf needs no `-e` flag here. The default `cycles` event is unavailable on this kernel and perf falls back to `task-clock` on its own.

### If you use `0x --kernel-tracing` instead

`0x` 6.0.0 corrupts its own capture. `platform/linux.js` runs `sed -i -e '/( __libc_start| LazyCompile |…|[unknown]|…)/d'` over the **binary** `perf.data`. In sed BRE, `[unknown]` is a _character class_ matching any of `u n k o w r s e d`, so it deletes nearly every newline-delimited chunk of the file and destroys the `PERF_RECORD_MMAP2` records — every native DSO then resolves as `/ (deleted)`. Apply `patches/0x-kernel-tracing.patch`, which makes that function a no-op (the filtering is purely cosmetic) and raises the hardcoded `-F 99` to `-F 999`:

```bash
cd "$(npm root -g)/0x"
cp platform/linux.js platform/linux.js.orig
patch -p0 < <path>/patches/0x-kernel-tracing.patch
```

`0x` takes the node binary from the first token after `--`, so the symbolizable copy from caveat 4 drops straight in:

```bash
sudo env PATH="$PATH" 0x --kernel-tracing --output-dir out -- \
  /tmp/prof/node-prof --import tsx profile-uniswap.ts --fuzz-runs 1000
```

Note this still runs the workload as root, so caveats 2 and 3 apply — which is why `record.sh` drives `perf` directly instead.
