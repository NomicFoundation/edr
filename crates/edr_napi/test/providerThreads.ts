// Regression tests for provider OS-thread reclamation.
//
// Every provider spawns a dedicated OS thread (#1486) that is joined only when
// its JS wrapper is finalized. EDR retains the subscription callback through a
// threadsafe function — a GC root V8 cannot trace through — so a callback that
// strongly reaches its wrapper roots the wrapper, its finalizer never runs, and
// the thread leaks. On macOS this exhausts the per-process thread cap.
//
// Thread counts are read from /proc/self/status, so these tests run on Linux
// only and skip elsewhere. They also need `global.gc`, i.e. mocha run with
// `--node-option expose-gc` (wired into .mocharc.cjs); without it they skip.

import { readFileSync } from "fs";
import { assert } from "chai";

import { EdrContext, Provider, SubscriptionEvent } from "..";
import {
  createGenericProvider,
  getContext,
  registerGenericProviderFactory,
} from "./helpers";

// Providers created per scenario. Large enough that a per-provider thread leak
// dwarfs the shared tokio pool and any measurement jitter.
const COUNT = 200;

// Threads still held, over the scenario's own baseline, that count as reclaimed.
// The no-leak case returns to ~0; a leak retains ~COUNT.
const RETAINED_THRESHOLD = 20;

function liveThreads(): number {
  const match = readFileSync("/proc/self/status", "utf-8").match(
    /^Threads:\s*(\d+)/m
  );
  if (match === null) {
    throw new Error("could not read Threads from /proc/self/status");
  }
  return Number(match[1]);
}

// Drive GC and let napi finalizers plus the async deallocator's thread-join
// run, returning the thread count once it stops falling.
async function settledThreads(): Promise<number> {
  let previous = Infinity;
  for (let round = 0; round < 40; round++) {
    global.gc!();
    await new Promise((resolve) => setImmediate(resolve));
    await new Promise((resolve) => setTimeout(resolve, 25));
    const current = liveThreads();
    if (current >= previous) {
      return current;
    }
    previous = current;
  }
  return liveThreads();
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

  for (let i = 0; i < COUNT; i++) {
    let subscriptionCallback: (event: SubscriptionEvent) => void;
    let bind: (provider: Provider) => void;

    if (capture === "weak") {
      // The HH2/HH3 guard: the callback reaches the wrapper only through a
      // WeakRef, assigned after construction.
      let weak: WeakRef<Provider> | undefined;
      subscriptionCallback = () => void weak?.deref();
      bind = (provider) => {
        weak = new WeakRef(provider);
      };
    } else {
      // An unguarded consumer: the callback reaches the wrapper strongly.
      const holder: { provider?: Provider } = {};
      subscriptionCallback = () => void holder.provider;
      bind = (provider) => {
        holder.provider = provider;
      };
    }

    bind(
      await createGenericProvider(context, {}, undefined, subscriptionCallback)
    );
  }

  const peak = liveThreads();
  const settled = await settledThreads();
  return { baseline, peak, settled };
}

function assertReclaimed({ baseline, peak, settled }: Measurement): void {
  // Guards against a false pass: the providers must actually have spawned
  // threads, or "reclaimed" would be meaningless.
  assert.isAtLeast(
    peak - baseline,
    COUNT * 0.8,
    "each provider should have spawned an OS thread"
  );
  assert.isAtMost(
    settled - baseline,
    RETAINED_THRESHOLD,
    `threads should be reclaimed (retained ${settled - baseline} of ${COUNT})`
  );
}

describe("provider OS-thread reclamation", function () {
  this.timeout(120000);

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

  // Pending until the retainer-side trampoline lands (Phase 2): today a
  // strongly-capturing callback roots the wrapper through EDR's threadsafe
  // function, so its thread leaks. Once EDR holds the wrapper weakly, an
  // unguarded consumer callback no longer matters and this must pass.
  it.skip("reclaims threads even for a strongly-referencing callback", async function () {
    assertReclaimed(await measureReclamation(context, "strong"));
  });
});
