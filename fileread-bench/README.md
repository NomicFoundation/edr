# Rust vs Node file-read benchmark

Verifies the claim that reading files is significantly faster in Rust than in Node.js/TypeScript, and probes _why_.

Both readers walk the same directory of `*.txt` files, read every file, and sum the total bytes (so nothing can be optimized away). Times are median of N iterations, warm page cache.

## Layout

- `generate.mjs` — creates `<dir>` with `<numFiles>` files of `<sizeBytes>` each.
- `rust/` — std-only Rust reader: sequential + threaded (4 / nproc / 64 threads).
- `ts/bench.mjs` — Node reader: sequential + `Promise.all` (bytes, utf-8, stat).
- `run.sh` — generates data, builds Rust, and runs everything.

## Run

```bash
./run.sh                # 4000 files x 32 KiB, 7 iters
./run.sh 20000 256 7    # many small files
```

Run the TS side under a specific pool size:

```bash
UV_THREADPOOL_SIZE=12 node ts/bench.mjs ./data 7
```

### Cold cache (macOS)

`run.sh` measures warm cache. To measure cold reads on a host Mac, use the purge-based runner, which repeats a purge + single cold read `repeats` times per strategy and reports median/min/max:

```bash
./run-cold-macos.sh 4000 32768 5    # numFiles, sizeBytes, repeats
```

Needs `purge` (Xcode Command Line Tools) and sudo. Only the first read after a purge is cold, so each strategy is measured single-shot and aggregated across repeats rather than looped in-process.

## Notes / caveats

- **Warm cache only.** This container has a read-only `/proc`, so the page cache can't be dropped. These numbers measure the _software dispatch/decoding path_, not cold-disk latency. Cold disk would add per-op latency where the 4-worker cap matters more — but also where Node and Rust converge toward disk-bound.
- The Rust build uses `lto=true, codegen-units=1, opt-level=3`.
- Rust reads into `Vec<u8>`; Node's `Promise.all` reads into `Buffer`. The utf-8 variant adds the main-thread decode Node does in real artifact loading.
