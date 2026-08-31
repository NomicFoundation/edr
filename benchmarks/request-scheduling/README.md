# Request-scheduling benchmarks

Standalone scripts that measure how the provider schedules JSON-RPC request
handling against interval mining. They exercise only the public JSON-RPC
surface of the locally built `crates/edr_napi` addon — no test hooks — so the
same script can be run on any branch to compare designs.

## read-latency-under-interval-mining.ts

Measures the latency of read-only requests (`eth_call`, `eth_getBalance`)
that are queued while mutating transactions are pending in the mempool and
interval mining is enabled — the normal operating mode of interval mining
(`auto_mine: false`).

```bash
pnpm -C crates/edr_napi build:dev   # release build of the napi addon
cd benchmarks/request-scheduling
node read-latency-under-interval-mining.ts                # 100 ms interval
node read-latency-under-interval-mining.ts --interval=1   # legal minimum
node read-latency-under-interval-mining.ts --no-interval  # baseline
```

Requires node >= 22.18 (built-in type stripping). `EDR_NAPI_PATH` can point
at another checkout's `crates/edr_napi` with a built addon.

### Sample results

One machine (dev container, aarch64), release addon builds, warm process; 6
pending transactions of 5.5M gas (one per block), 6 concurrent `eth_call`s of
2M gas (~100 ms each) plus 3 `eth_getBalance`s:

| scenario           | `main` (fair mutex)  | provider event loop  |
| ------------------ | -------------------- | -------------------- |
| no interval mining | 196 ms               | 166 ms               |
| interval 100 ms    | 234 ms · 1 block     | 450 ms · 5 blocks    |
| interval 1 ms      | 281 ms · 1 block     | ~1.6 s · ~250 blocks |

("all reads answered in" · blocks mined during the drain.)

The block counts show the trade-off from both sides: `main`'s fair mutex
serves all queued reads before the mining task's next turn (flat read
latencies, block production delayed by the burst), while the event loop
prioritizes the mining timer (blocks stay on schedule; once a mining pass
outlasts the interval, queued reads drain roughly one per pass, with even
trivial `eth_getBalance` requests waiting behind the whole queue).
