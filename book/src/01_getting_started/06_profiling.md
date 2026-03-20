# Profiling

## Perf/DTrace

The [cargo-flamegraph](https://github.com/flamegraph-rs/flamegraph) tool can be used to collect performance profiling data using [perf](<https://en.wikipedia.org/wiki/Perf_(Linux)>) on Linux and [DTrace](https://en.wikipedia.org/wiki/DTrace) on MacOS/Windows and then visualize it as a flamegraph. This only works when executing `edr` from Rust, so it's mostly used to profile the [scenarios](../02_development/01_tools.md#scenarios) in the repository.

If you are using devcontainers, make sure to elevate the privileges of the container to allow perf to work correctly. Edit the `devcontainer.json` file to uncomment the following line:

```json
  "capAdd": [
    // Enable for profiling with `perf`
    "SYS_ADMIN"
  ],
```

Then rebuild the container.

### Instructions

Install the `cargo-flamegraph` tool by running:

```bash
cargo install flamegraph
```

(If you're on Linux, check the [readme](https://github.com/flamegraph-rs/flamegraph?tab=readme-ov-file#installation) for distro specific instructions.)

Then create the flamegraph from the repo root, for example for the `seaport` scenario with:

```bash
CARGO_PROFILE_RELEASE_DEBUG=true cargo flamegraph -o flamegraph_seaport.svg --root --release -- scenario crates/tools/scenarios/neptune-mutual-blue-protocol_8db6480.jsonl.gz
```

The flamegraph will be saved to `flamegraph_seaport.svg`.

## Combined JS + Native Flamegraph (Hardhat)

When EDR is used from JavaScript via Hardhat, it's possible to generate a combined flamegraph that shows both JavaScript call frames (Hardhat, ethers.js, test code) and native Rust frames (EDR internals) in a single interactive view.

This uses [`0x`](https://github.com/davidmarkclements/0x) with its `--kernel-tracing` mode, which runs the V8 tick profiler and Linux `perf` simultaneously. The `--always-turbofan` flag forces V8 to JIT-compile all JavaScript functions, making them visible to the kernel-level profiler. Without it, unoptimized and lazily-compiled functions (which cover the majority of Hardhat's code) appear only as anonymous `ByteCodeHandler` frames.

### Prerequisites

Install the required tools:

```bash
# Linux perf
sudo apt-get install linux-perf

# 0x flamegraph tool
npm install -g 0x
```

### Instructions

From the repository containing the Hardhat project (e.g. `openzeppelin-contracts`), install dependencies and compile contracts if you haven't already:

```bash
npm install
npm run compile
```

Then run the profiler. The `--` separator passes the remaining arguments to Node directly:

```bash
sudo env PATH="$PATH" 0x \
  --kernel-tracing \
  --output-dir flamegraph-out \
  -- node --always-turbofan node_modules/.bin/hardhat test
```

To profile a specific test file instead of the full suite:

```bash
sudo env PATH="$PATH" 0x \
  --kernel-tracing \
  --output-dir flamegraph-out \
  -- node --always-turbofan node_modules/.bin/hardhat test test/token/ERC20/ERC20.test.js
```

> **Note:** `sudo env PATH="$PATH"` is required because `sudo` resets `PATH` by default, which prevents it from finding `node` and `0x` when installed via nvm.

Once complete, open `flamegraph-out/flamegraph.html` in a browser. The flamegraph is interactive — click any frame to zoom in.

### Why `--always-turbofan`?

By default, V8 only JIT-compiles functions that are called frequently enough to warrant optimisation. Functions that run once during initialisation (e.g. config parsing, provider setup) remain in the interpreter and are invisible to the kernel-level profiler — they appear as anonymous `ByteCodeHandler` frames.

`--always-turbofan` instructs V8 to compile every function with the Turbofan optimising compiler, giving each its own named machine-code stub that perf can identify. This makes the flamegraph significantly more readable at the cost of a slower startup time and slightly altered runtime characteristics.

Be aware that this flag fundamentally changes the performance characteristics of the application, so should probably be avoided when attempting to improve the performance of an application. See the [0x kernel tracing docs](https://github.com/davidmarkclements/0x/blob/master/docs/kernel-tracing.md) for more details.

## Event Tracing

It's possible to profile the execution of `edr` by collecting [execution traces](https://docs.rs/tracing/latest/tracing/) and then turning them into flamegraphs. This has the advantage that the contents of the flamegraph can be filtered on the tracing level, and it works when EDR is ran from JS.

### Instructions

```bash
pnpm build:tracing
```

When you now run `edr`, it will generate a `tracing.folded` file in the current working directory. Once the profiling run has completed, we can use [`inferno`](https://docs.rs/tracing-flame/latest/tracing_flame/#generating-the-image) to generate flamegraphs from the collected data. To install `inferno`, run:

```bash
cargo install inferno
```

When we want to analyze the run with its exact order preserved, run:

```bash
cat tracing.folded | inferno-flamegraph --flamechart > tracing-flamechart.svg
```

Alternatively, when we don't care about those details, a flamegraph with identical stack frames collapsed can be generated by running:

```bash
cat tracing.folded | inferno-flamegraph > tracing-flamegraph.svg
```
