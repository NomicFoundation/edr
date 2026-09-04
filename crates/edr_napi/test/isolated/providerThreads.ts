// Regression test for provider OS-thread reclamation.
//
// A provider's OS thread (#1486) is joined only when its JS wrapper is
// finalized, and EDR holds the subscription callback through a threadsafe
// function V8 cannot trace. A callback capturing the wrapper used to root it
// and leak the thread; the `createProvider` override in index.js prevents
// that, and this asserts threads are reclaimed either way. Runs in its own
// process (`test:isolated`) for a clean thread baseline; Linux-only
// (`/proc/self/status`) and needs `expose-gc`.

import { readFileSync } from "fs";
import { assert } from "chai";

import { EdrContext, Provider, SubscriptionEvent } from "../..";
import {
  createGenericProvider,
  getContext,
  registerGenericProviderFactory,
} from "../helpers";

// Enough providers that a per-provider leak dwarfs measurement jitter.
const COUNT = 200;

// A leak retains ~COUNT threads and the fix ~0, so this stays tight.
const RETAINED_THRESHOLD = 20;

// Bounds the wait for reclamation; a real leak rides it out to a failure.
const SETTLE_BUDGET_MS = 20_000;

function liveThreads(): number {
  const match = readFileSync("/proc/self/status", "utf-8").match(
    /^Threads:\s*(\d+)/m
  );
  if (match === null) {
    throw new Error("could not read Threads from /proc/self/status");
  }
  return Number(match[1]);
}

// Forces GC and yields so finalizers and thread joins can run.
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
// their wrappers in the given way.
async function measureReclamation(
  context: EdrContext,
  capture: "weak" | "strong"
): Promise<Measurement> {
  const baseline = liveThreads();

  // Held until the peak is measured, so the peak is deterministic.
  let providers: Provider[] | null = [];

  for (let i = 0; i < COUNT; i++) {
    let subscriptionCallback: (event: SubscriptionEvent) => void;
    let bind: (created: Provider) => void;

    if (capture === "weak") {
      // Hardhat's WeakRef guard.
      let weak: WeakRef<Provider> | undefined;
      subscriptionCallback = () => void weak?.deref();
      bind = (created) => {
        weak = new WeakRef(created);
      };
    } else {
      // An unguarded consumer.
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

  providers = null;

  const settled = await settleToBaseline(baseline);
  return { baseline, peak, settled };
}

function assertReclaimed({ baseline, peak, settled }: Measurement): void {
  const spawned = peak - baseline;
  const retained = settled - baseline;

  // Guards against a false pass: the providers must have spawned threads.
  assert(
    spawned >= COUNT * 0.8,
    `providers did not spawn their threads: only ${spawned} of ~${COUNT} appeared at peak (expected at least ${Math.floor(
      COUNT * 0.8
    )})`
  );

  assert(
    retained <= RETAINED_THRESHOLD,
    `provider threads were not reclaimed: ${retained} of ${COUNT} still alive after release (expected at most ${RETAINED_THRESHOLD})`
  );
}

describe("provider OS-thread reclamation", function () {
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

  // The `createProvider` override in index.js makes this hold too.
  it("reclaims threads even for a strongly-referencing callback", async function () {
    assertReclaimed(await measureReclamation(context, "strong"));
  });
});
