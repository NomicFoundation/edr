// Regression test for provider OS-thread reclamation.
//
// Every provider spawns a dedicated OS thread (#1486) that is joined only when
// its JS wrapper is finalized. EDR retains the subscription callback through a
// threadsafe function — a GC root V8 cannot trace through — so a callback that
// strongly reaches its wrapper roots the wrapper, its finalizer never runs, and
// the thread leaks. On macOS this exhausts the per-process thread cap.
//
// Run out of the main suite, in its own process (see the `test:threads` script),
// so the thread-count baseline is this test's alone — no threads left over from
// other test files. It reads `/proc/self/status`, so it runs on Linux only and
// skips elsewhere; it needs `global.gc` (mocha `--node-option expose-gc`) and
// skips without it.

import { readFileSync } from "fs";
import { assert } from "chai";

import { EdrContext, Provider, SubscriptionEvent } from "../..";
import {
  createGenericProvider,
  getContext,
  registerGenericProviderFactory,
} from "../helpers";

// Providers created per scenario. Large enough that a per-provider thread leak
// dwarfs the shared tokio pool and any measurement jitter.
const COUNT = 200;

// Threads still held, over the scenario's own baseline, that count as reclaimed.
// The gap this must resolve is enormous — a leak retains ~COUNT, the fix ~0 —
// and a regression leaks *every* provider (all go through the same path), so
// this stays tight; `settleToBaseline` waits out reclamation lag rather than
// this tolerating it.
const RETAINED_THRESHOLD = 20;

// How long `settleToBaseline` waits for reclamation before giving up. Drains in
// one batch when unloaded; the budget only bounds the wait on a busy runner.
const SETTLE_BUDGET_MS = 20_000;

// The process's current live OS-thread count (Linux).
function liveThreads(): number {
  const match = readFileSync("/proc/self/status", "utf-8").match(
    /^Threads:\s*(\d+)/m
  );
  if (match === null) {
    throw new Error("could not read Threads from /proc/self/status");
  }
  return Number(match[1]);
}

// Drives GC and yields so napi finalizers and the async deallocator's thread
// joins can run, until the count returns to within RETAINED_THRESHOLD of
// `baseline` or the budget elapses. Forcing GC makes collection deterministic;
// the budget only bounds how long we wait for the joins to drain, so a busy
// runner makes this slower rather than flaky. A real leak never drains and rides
// the budget out to a failing assertion.
async function settleToBaseline(baseline: number): Promise<number> {
  const deadline = Date.now() + SETTLE_BUDGET_MS;
  let count = liveThreads();
  while (count - baseline > RETAINED_THRESHOLD && Date.now() < deadline) {
    for (let cycle = 0; cycle < 10; cycle++) {
      global.gc!();
      await new Promise((resolve) => setImmediate(resolve));
      await new Promise((resolve) => setTimeout(resolve, 15));
    }
    count = liveThreads();
  }
  return count;
}

interface Measurement {
  baseline: number;
  peak: number;
  settled: number;
}

// Creates and releases COUNT providers whose subscription callbacks reference
// their wrappers in the given way, then reports how many threads were spawned
// and how many survived reclamation.
async function measureReclamation(
  context: EdrContext,
  capture: "weak" | "strong"
): Promise<Measurement> {
  const baseline = liveThreads();

  // Hold every provider alive until the peak is measured, so the peak is
  // deterministic regardless of when GC runs during creation.
  let providers: Provider[] | null = [];

  for (let i = 0; i < COUNT; i++) {
    let subscriptionCallback: (event: SubscriptionEvent) => void;
    let bind: (created: Provider) => void;

    if (capture === "weak") {
      // The HH2/HH3 guard: the callback reaches the provider only through a
      // WeakRef, assigned after construction.
      let weak: WeakRef<Provider> | undefined;
      subscriptionCallback = () => void weak?.deref();
      bind = (created) => {
        weak = new WeakRef(created);
      };
    } else {
      // An unguarded consumer: the callback reaches the provider strongly.
      const holder: { provider?: Provider } = {};
      subscriptionCallback = () => void holder.provider;
      bind = (created) => {
        holder.provider = created;
      };
    }

    const provider = await createGenericProvider(
      context,
      {},
      undefined,
      subscriptionCallback
    );
    bind(provider);
    providers.push(provider);
  }

  const peak = liveThreads();

  // Release every provider; only the subscription callback's reference (weak,
  // or — for `strong` — routed through EDR's trampoline) remains.
  providers = null;

  const settled = await settleToBaseline(baseline);
  return { baseline, peak, settled };
}

function assertReclaimed({ baseline, peak, settled }: Measurement): void {
  const spawned = peak - baseline;
  const retained = settled - baseline;

  // `assert(condition, message)` reports only the message on failure, avoiding
  // chai's equality-style `expected/actual` diff — which here would misleadingly
  // frame the threshold as a target rather than an upper bound.

  // Guards against a false pass: the providers must actually have spawned
  // threads, or "reclaimed" would be meaningless.
  assert(
    spawned >= COUNT * 0.8,
    `providers did not spawn their threads: only ${spawned} of ~${COUNT} appeared at peak (expected at least ${Math.floor(
      COUNT * 0.8
    )})`
  );

  // A leak retains ~COUNT threads; the fix retains ~0.
  assert(
    retained <= RETAINED_THRESHOLD,
    `provider threads were not reclaimed: ${retained} of ${COUNT} still alive after release (expected at most ${RETAINED_THRESHOLD})`
  );
}

describe("provider OS-thread reclamation", function () {
  // Generous: covers up to two full settle budgets plus provider creation.
  this.timeout(SETTLE_BUDGET_MS * 2 + 60_000);

  before(function () {
    if (process.platform !== "linux" || typeof global.gc !== "function") {
      this.skip();
    }
  });

  let context: EdrContext;
  before(async function () {
    context = getContext();
    await registerGenericProviderFactory(context);
  });

  it("reclaims threads for a weakly-referencing subscription callback", async function () {
    assertReclaimed(await measureReclamation(context, "weak"));
  });

  // `createProvider`'s wrapper (see index.js) stores the callback on the
  // provider and hands the binding a trampoline that reaches it only weakly,
  // so even a strongly-capturing consumer callback no longer roots the
  // provider.
  it("reclaims threads even for a strongly-referencing callback", async function () {
    assertReclaimed(await measureReclamation(context, "strong"));
  });
});
