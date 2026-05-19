#!/usr/bin/env -S node --expose-gc --max-old-space-size=8192

// Force-reproduce the leaked-IntervalMiner-Drop deadlock at a controlled
// moment by calling V8's `global.gc()` after creating N providers with
// interval mining enabled.
//
// Prerequisites:
//   1. Build the EDR napi crate:
//        pnpm -C crates/edr_napi build:dev
//      This produces crates/edr_napi/edr.<platform>.node + index.js
//
//   2. Run on any OS (Linux/macOS/Windows) — the deadlock is OS-independent:
//        node --expose-gc js/deadlock-repro/leaked-interval-miner.mjs [N]
//      Default N=20.
//
// Outcomes:
//   - Prints "Survived Nms after gc()" within a few seconds → no deadlock
//   - Script HANGS at "Waiting Nms for deferred finalizers" → deadlock
//     reproduced. Wrap with `timeout 30s` (or use js/deadlock-repro/run.sh):
//        timeout 30s node --expose-gc js/deadlock-repro/leaked-interval-miner.mjs 20
//        echo "exit code: $?   (124 = hang, 0 = clean)"

import {
  EdrContext,
  GENERIC_CHAIN_TYPE,
  L1_CHAIN_TYPE,
  ContractDecoder,
  MineOrdering,
  genericChainProviderFactory,
  l1GenesisState,
  l1HardforkFromString,
  l1HardforkLatest,
  l1HardforkToString,
  l1ProviderFactory,
} from "../../crates/edr_napi/index.js";

const N = parseInt(process.argv[2] ?? "20", 10);
const INTERVAL_MINING = !process.argv.includes("--no-interval");

// Validate --expose-gc
const gc = globalThis.gc;
if (typeof gc !== "function") {
  console.error("ERROR: run with `node --expose-gc`");
  console.error("Example: node --expose-gc js/deadlock-repro/leaked-interval-miner.mjs 20");
  process.exit(2);
}

// ─── Provider config ──────────────────────────────────────────────────────
// Mirrors the canonical config from crates/edr_napi/test/provider.ts.
// With --no-interval: autoMine ON, no background task (control case).
// Default: autoMine OFF, mining.interval=50ms — each provider gets its own
// IntervalMiner background task.

const hardfork = l1HardforkToString(l1HardforkLatest());
const genesisState = l1GenesisState(l1HardforkFromString(hardfork));

const providerConfig = {
  allowBlocksWithSameTimestamp: false,
  allowUnlimitedContractSize: true,
  bailOnCallFailure: false,
  bailOnTransactionFailure: false,
  chainId: 123n,
  chainOverrides: [],
  coinbase: new Uint8Array(
    Buffer.from("0000000000000000000000000000000000000000", "hex"),
  ),
  defaultTransactionGasLimit: 300_000_000n,
  genesisState,
  hardfork,
  initialBlobGas: {
    gasUsed: 0n,
    excessGas: 0n,
  },
  initialParentBeaconBlockRoot: new Uint8Array(
    Buffer.from(
      "0000000000000000000000000000000000000000000000000000000000000000",
      "hex",
    ),
  ),
  minGasPrice: 0n,
  mining: {
    // interval mining ON (default): autoMine OFF, 50ms interval.
    // Each provider gets its own IntervalMiner background task, which is
    // the load-bearing piece of the deadlock. Short interval maximises
    // time spent in blocking TSFN calls and the probability of GC firing
    // mid-mine.
    //
    // interval mining OFF (--no-interval): autoMine ON, no background task.
    // Control case — expected to GC cleanly with no deadlock.
    autoMine: !INTERVAL_MINING,
    blockGasLimit: 300_000_000n,
    ...(INTERVAL_MINING ? { interval: 50n } : {}),
    memPool: { order: MineOrdering.Priority },
  },
  network: {
    genesisBlobGas: {
      gasUsed: 0n,
      excessGas: 0n,
    },
    genesisBlockGasLimit: 300_000_000n,
  },
  networkId: 123n,
  observability: {},
  ownedAccounts: [
    "0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80",
  ],
  precompileOverrides: [],
};

const loggerConfig = {
  // Must be true — when false, logger.rs short-circuits before calling print_line_fn,
  // so the IntervalMiner task never makes any blocking-on-JS TSFN calls and the
  // deadlock cycle cannot form.
  enable: true,
  decodeConsoleLogInputsCallback: (_inputs) => [],
  printLineCallback: (_message, _replace) => {},
};

const subscriptionConfig = {
  subscriptionCallback: (_event) => {},
};

// ─── Test body ────────────────────────────────────────────────────────────

async function main() {
  console.log(`leaked-interval-miner repro: N=${N}`);
  console.log(`Node: ${process.version}, platform: ${process.platform}-${process.arch}`);
  console.log(`Hardfork: ${hardfork}`);
  console.log("");

  console.log("Initializing EdrContext...");
  const ctx = new EdrContext();
  await ctx.registerProviderFactory(GENERIC_CHAIN_TYPE, genericChainProviderFactory());
  await ctx.registerProviderFactory(L1_CHAIN_TYPE, l1ProviderFactory());
  const contractDecoder = new ContractDecoder();

  const modeLabel = INTERVAL_MINING
    ? `interval mining ON (interval=${providerConfig.mining.interval}ms)`
    : "interval mining OFF (autoMine, no background task — control case)";
  console.log(`Creating ${N} providers with ${modeLabel}...`);
  for (let i = 0; i < N; i++) {
    let p = await ctx.createProvider(
      GENERIC_CHAIN_TYPE,
      providerConfig,
      loggerConfig,
      subscriptionConfig,
      contractDecoder,
    );

    // Touch the provider once so the IntervalMiner has actually started.
    await p.handleRequest(
      JSON.stringify({ jsonrpc: "2.0", id: i, method: "eth_blockNumber" }),
    );

    // Drop the JS reference without calling any napi-level cleanup.
    // HH2 does `delete this.provider`; HH3's close() does
    // `this.#provider = undefined`. Both drop the JS reference and rely on
    // V8 GC to run the Rust finalizer — neither signals the tokio runtime
    // or IntervalMiner directly.
    p = null;

    if ((i + 1) % 5 === 0 || i + 1 === N) {
      console.log(`  ${i + 1}/${N} created and abandoned`);
    }
  }

  console.log("");
  console.log("All providers dropped.");
  console.log("IntervalMiner tasks are still alive in their respective tokio runtimes.");
  console.log("");

  console.log(`Requesting GC at ${new Date().toISOString()}...`);
  console.log("NOTE: global.gc() marks objects as unreachable but V8 schedules");
  console.log("napi_finalize callbacks as DEFERRED work — they run on the JS");
  console.log("thread when the callstack unwinds and the event loop gets its");
  console.log("next turn. The deadlock therefore fires in the await below, not");
  console.log("inside the gc() call itself.");
  console.log("");

  const before = Date.now();
  gc(); // marks unreachable objects; napi_finalize runs after callstack unwinds
  const gcCallMs = Date.now() - before;
  console.log(`gc() call returned in ${gcCallMs}ms (finalizers not yet run).`);
  console.log("");

  // Allow the event loop to pump so that deferred napi_finalize callbacks
  // execute. If any IntervalMiner::Drop deadlocks, the process will hang
  // here and the outer `timeout` wrapper will catch it (exit 124).
  const WAIT_MS = 5_000;
  console.log(`Waiting ${WAIT_MS}ms for deferred finalizers to run...`);
  console.log("  → If the script hangs here, the deadlock has fired.");
  console.log("  → Wrap with 'timeout 30s' (run.sh) to detect.");
  console.log("");
  await new Promise((resolve) => setTimeout(resolve, WAIT_MS));

  const elapsed = Date.now() - before;
  console.log(`Survived ${elapsed}ms after gc(). Process did not hang.`);
  console.log("Check stderr for '[edr_provider] WARNING' lines:");
  console.log("  WARNING present → deadlock fired but was bounded by the Drop timeout.");
  console.log("  No WARNING      → no deadlock. Either:");
  console.log("    - This Node version has the upstream fix (≥ 24.13.1 LTS, or ≥ 25)");
  console.log("    - The architectural conditions for the deadlock weren't met");
  console.log("    - N=" + N + " wasn't enough to trigger; try bumping it");
}

main().catch((err) => {
  console.error("repro failed:", err);
  process.exit(1);
});
