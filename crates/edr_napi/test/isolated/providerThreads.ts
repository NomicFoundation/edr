// Regression tests for provider OS-thread reclamation and callback ownership.
//
// A provider's OS thread (#1486) is joined only when its JS wrapper is
// finalized, and a threadsafe function holds its JS function in a global
// handle — a root V8 traces from rather than an edge it can trace to. Any
// consumer callback that reached back to its provider used to be rooted that
// way and took the provider with it, leaking the thread. `src/callback.rs`
// gives each threadsafe function a trampoline instead and makes the
// provider's own JS object own the callback, so every reclamation case below
// settles to the baseline.
//
// Two axes are covered. `capture` varies how a subscription callback reaches
// its provider: not at all (`weak`, via the `WeakRef` guard consumers needed
// before), directly (`strong`), and through a wrapper object the consumer
// holds (`wrapper`) — what a consumer such as Hardhat actually passes in, and
// the case a fix keyed on the provider alone has to be checked against.
// `callback` then applies the `wrapper` shape to each of the other callbacks a
// provider owns, since each used to be rooted by its own threadsafe function
// and leaked independently of the subscription one.
//
// A final case covers the other direction: with the provider held, its
// attached callback must survive garbage collection and keep delivering.
//
// Runs in its own process (`test:isolated`) for a clean thread baseline;
// Linux-only (`/proc/self/status`) and needs `expose-gc`.

import { readFileSync } from "fs";
import { assert } from "chai";

import {
  CallOverrideResult,
  EdrContext,
  LoggerConfig,
  Provider,
  ProviderConfig,
  SubscriptionEvent,
} from "../..";
import {
  createGenericProvider,
  getContext,
  intervalMiningConfig,
  registerGenericProviderFactory,
  silentLoggerConfig,
} from "../helpers";

// Enough providers that a per-provider leak dwarfs measurement jitter.
const COUNT = 200;

// A leak retains ~COUNT threads and the fix ~0, so this stays tight.
const RETAINED_THRESHOLD = 20;

// Bounds the wait for reclamation; a real leak rides it out to a failure.
const SETTLE_BUDGET_MS = 20_000;

/**
 * Interval, in milliseconds, for the cases that mine from the provider's own
 * thread.
 */
const MINING_INTERVAL_MS = 50n;

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

/**
 * Stands in for a consumer-side provider wrapper such as Hardhat's: an object
 * the consumer holds that owns the provider and exposes its own surface.
 */
class ProviderWrapper {
  constructor(private readonly _provider: Provider) {}

  public handleRequest(request: string): Promise<unknown> {
    return this._provider.handleRequest(request);
  }
}

/** What a capturing callback closes over, filled in once the provider exists. */
interface Holder {
  wrapper?: ProviderWrapper;
}

/** How a subscription callback reaches back to its provider. */
type Capture = "weak" | "strong" | "wrapper";

/** Which of a provider's callbacks carries the capture. */
type Callback =
  | "subscription"
  | "printLine"
  | "decodeConsoleLogInputs"
  | "callOverride"
  | "coverage"
  | "gasReport";

/** How a provider is built so that `callback` closes over `holder`. */
interface Subject {
  overrides: Partial<ProviderConfig>;
  loggerConfig: LoggerConfig;
  subscriptionCallback: (event: SubscriptionEvent) => void;
  /** Callbacks set on a provider that already exists, such as the override. */
  install?: (provider: Provider) => Promise<void>;
}

// Every callback below is built by its own function, and each function
// receives only what its callback is meant to capture. That is not tidiness:
// closures created in the same function share one scope, and V8 keeps a
// variable in that scope alive if *any* closure there reads it. Building a
// capturing callback beside a supposedly non-capturing one would hand the
// second one the first one's `holder` through the shared scope. Every case
// would then measure the same strong capture, which is why `weak` has to be
// built by a function that never sees a `Holder` at all.
//
// Rust's closures have the opposite default: a `move` closure captures
// individual fields rather than the whole variable, which is why
// `ReferenceOwner::reference` in `src/callback.rs` is a method rather than a
// field read.

/** Module scope, so they share no scope with any `Holder`. */
const NO_SUBSCRIPTION_CALLBACK = (): void => {};
const NO_DECODE_CONSOLE_LOG_INPUTS_CALLBACK = (): string[] => [];
const NO_PRINT_LINE_CALLBACK = (): void => {};

/** Hardhat's `WeakRef` guard: never reaches the provider strongly. */
function weakSubscription(): Pick<Subject, "subscriptionCallback" | "install"> {
  let weak: WeakRef<ProviderWrapper> | undefined;

  return {
    subscriptionCallback: () => void weak?.deref(),
    install: (provider) => {
      weak = new WeakRef(new ProviderWrapper(provider));
      return Promise.resolve();
    },
  };
}

/** An unguarded consumer: the callback reaches the provider through `holder`. */
function capturingSubscription(
  holder: Holder
): (event: SubscriptionEvent) => void {
  return () => void holder.wrapper;
}

function capturingDecodeConsoleLogInputs(
  holder: Holder
): (inputs: ArrayBuffer[]) => string[] {
  return () => {
    void holder.wrapper;
    return [];
  };
}

function capturingPrintLine(
  holder: Holder
): (message: string, replace: boolean) => void {
  return () => {
    void holder.wrapper;
  };
}

// The callback not under test comes from module scope, so this function
// creates no closure that could capture `holder`. Otherwise both logger cases
// would measure the same capture.
function capturingLogger(
  which: "printLine" | "decodeConsoleLogInputs",
  holder: Holder
): LoggerConfig {
  return {
    // Resolved whether or not the logger is enabled, which is exactly why a
    // consumer who turned logging off still used to pin their provider.
    enable: false,
    decodeConsoleLogInputsCallback:
      which === "decodeConsoleLogInputs"
        ? capturingDecodeConsoleLogInputs(holder)
        : NO_DECODE_CONSOLE_LOG_INPUTS_CALLBACK,
    printLineCallback:
      which === "printLine"
        ? capturingPrintLine(holder)
        : NO_PRINT_LINE_CALLBACK,
  };
}

function capturingCoverage(holder: Holder): Partial<ProviderConfig> {
  return {
    observability: {
      codeCoverage: {
        onCollectedCoverageCallback: (): Promise<void> => {
          void holder.wrapper;
          return Promise.resolve();
        },
      },
    },
  };
}

function capturingGasReport(holder: Holder): Partial<ProviderConfig> {
  return {
    observability: {
      gasReport: {
        onCollectedGasReportCallback: (): Promise<void> => {
          void holder.wrapper;
          return Promise.resolve();
        },
      },
    },
  };
}

function capturingCallOverride(
  holder: Holder
): (provider: Provider) => Promise<void> {
  return async (provider) => {
    await provider.setCallOverrideCallback(
      (): Promise<CallOverrideResult | undefined> => {
        void holder.wrapper;
        return Promise.resolve(undefined);
      }
    );
  };
}

function miningOverrides(subscribed: boolean): Partial<ProviderConfig> {
  return subscribed ? { mining: intervalMiningConfig(MINING_INTERVAL_MS) } : {};
}

// Composes a subject without creating any closure of its own, so nothing it
// hands to EDR can capture `holder` except the callback under test.
function subjectFor(
  callback: Callback,
  capture: Capture,
  holder: Holder,
  subscribed: boolean
): Subject {
  const overrides = miningOverrides(subscribed);

  switch (callback) {
    case "subscription":
      return capture === "weak"
        ? {
            overrides,
            loggerConfig: silentLoggerConfig(),
            ...weakSubscription(),
          }
        : {
            overrides,
            loggerConfig: silentLoggerConfig(),
            subscriptionCallback: capturingSubscription(holder),
          };

    case "printLine":
    case "decodeConsoleLogInputs":
      return {
        overrides,
        loggerConfig: capturingLogger(callback, holder),
        subscriptionCallback: NO_SUBSCRIPTION_CALLBACK,
      };

    case "callOverride":
      return {
        overrides,
        loggerConfig: silentLoggerConfig(),
        subscriptionCallback: NO_SUBSCRIPTION_CALLBACK,
        install: capturingCallOverride(holder),
      };

    case "coverage":
      return {
        overrides: { ...overrides, ...capturingCoverage(holder) },
        loggerConfig: silentLoggerConfig(),
        subscriptionCallback: NO_SUBSCRIPTION_CALLBACK,
      };

    case "gasReport":
      return {
        overrides: { ...overrides, ...capturingGasReport(holder) },
        loggerConfig: silentLoggerConfig(),
        subscriptionCallback: NO_SUBSCRIPTION_CALLBACK,
      };
  }
}

// Creates and releases COUNT providers whose `callback` reaches back to them,
// and reports how many of their threads survive being released.
//
// `subscribed` gives each provider an active `newHeads` subscription and
// interval mining, so its callbacks are being invoked from its own OS thread
// right up to the moment the consumer lets go.
async function measureReclamation(
  context: EdrContext,
  callback: Callback,
  capture: Capture = "wrapper",
  subscribed = false
): Promise<Measurement> {
  const baseline = liveThreads();

  // Held until the peak is measured, so the peak is deterministic. Holds what
  // a consumer would hold: the wrapper owning the provider.
  let held: object[] | null = [];

  for (let i = 0; i < COUNT; i++) {
    const holder: Holder = {};
    const subject = subjectFor(callback, capture, holder, subscribed);

    const provider = await createGenericProvider(
      context,
      subject.overrides,
      subject.loggerConfig,
      subject.subscriptionCallback
    );

    await subject.install?.(provider);

    if (subscribed) {
      await provider.handleRequest(
        JSON.stringify({
          id: 1,
          jsonrpc: "2.0",
          method: "eth_subscribe",
          params: ["newHeads"],
        })
      );
    }

    // The consumer's only reference, and what every capturing callback above
    // closes over.
    holder.wrapper = new ProviderWrapper(provider);
    held.push(holder.wrapper);
  }

  const peak = liveThreads();

  held = null;

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
  // Each reclamation case creates and releases COUNT providers, and a
  // failing one rides out the settle budget.
  this.timeout(SETTLE_BUDGET_MS * 12 + 300_000);

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

  describe("subscription callback", function () {
    it("reclaims threads for a weakly-referencing callback", async function () {
      assertReclaimed(
        await measureReclamation(context, "subscription", "weak")
      );
    });

    it("reclaims threads for a strongly-referencing callback", async function () {
      assertReclaimed(
        await measureReclamation(context, "subscription", "strong")
      );
    });

    // The strong reference runs through a consumer-side wrapper rather than
    // pointing at the provider directly, so a fix keyed on the callback's own
    // view of the provider would not cover it.
    it("reclaims threads for a callback capturing a provider wrapper", async function () {
      assertReclaimed(
        await measureReclamation(context, "subscription", "wrapper")
      );
    });

    // The same, while each provider is delivering events from its own thread
    // rather than sitting idle.
    it("reclaims threads for a live subscription captured through a wrapper", async function () {
      assertReclaimed(
        await measureReclamation(context, "subscription", "wrapper", true)
      );
    });
  });

  // Every other callback a provider owns used to be rooted by its own
  // threadsafe function, so each leaked on its own and needs its own case.
  describe("other provider callbacks", function () {
    const OTHERS: Callback[] = [
      "printLine",
      "decodeConsoleLogInputs",
      "callOverride",
      "coverage",
      "gasReport",
    ];

    for (const callback of OTHERS) {
      it(`reclaims threads for a ${callback} callback capturing a provider wrapper`, async function () {
        assertReclaimed(await measureReclamation(context, callback));
      });
    }
  });

  // Reclamation alone cannot tell a working attachment from a botched one. An
  // attachment that released the reference's count without an owner taking
  // the callback would leave it collectable, so every case above would pass
  // while events silently stopped. This case pins the other half of the
  // contract: the callback lives exactly as long as its provider.
  //
  // Declared after the reclamation cases on purpose. This provider keeps
  // interval mining until the process exits, which would inflate their
  // baselines.
  describe("callback ownership", function () {
    it("keeps delivering events across GC while the provider is held", async function () {
      const EXPECTED_AFTER_GC = 5;
      const DELIVERY_BUDGET_MS = 10_000;

      let events = 0;

      const provider = await createGenericProvider(
        context,
        { mining: intervalMiningConfig(MINING_INTERVAL_MS) },
        silentLoggerConfig(),
        () => {
          events += 1;
        }
      );

      await provider.handleRequest(
        JSON.stringify({
          id: 1,
          jsonrpc: "2.0",
          method: "eth_subscribe",
          params: ["newHeads"],
        })
      );

      // Nothing on the consumer side holds the callback: only the provider's
      // own object does. A broken attachment would let this GC collect it
      // and silence the subscription.
      for (let cycle = 0; cycle < 10; cycle++) {
        global.gc!();
        await new Promise((resolve) => setImmediate(resolve));
      }

      const seenBeforeGc = events;
      const deadline = Date.now() + DELIVERY_BUDGET_MS;
      while (
        events < seenBeforeGc + EXPECTED_AFTER_GC &&
        Date.now() < deadline
      ) {
        await new Promise((resolve) => setTimeout(resolve, 15));
      }

      assert.isAtLeast(
        events,
        seenBeforeGc + EXPECTED_AFTER_GC,
        "events stopped arriving after GC; the callback lost its owner"
      );

      // Reading the provider here keeps it alive through the GC loop above,
      // so the case cannot pass by the provider being collected early.
      const holder = (provider as unknown as Record<string, unknown>)
        .__edrCallbacks as Record<string, unknown>;
      assert.isFunction(holder.subscription);
    });
  });
});
