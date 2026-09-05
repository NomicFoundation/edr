// @ts-check
// Hand-written entry point wrapping the generated N-API binding (`binding.js`).
//
// A provider owns a dedicated OS thread that is only reclaimed when its
// JavaScript wrapper is garbage-collected. EDR retains the subscription
// callback through a threadsafe function — a reference V8 cannot trace
// through — so a callback that reaches back to the provider would root it and
// leak the thread. To keep that guarantee off consumers, `createProvider`
// keeps the callback in a WeakMap keyed by the provider and hands the binding a
// trampoline that reaches the provider only weakly.
//
// `// @ts-check` above type-checks this file against `binding.d.ts` (see
// `tsconfig.check.json`), so a change to `createProvider`'s signature that this
// wrapper no longer matches fails the build.

const binding = require("./binding");

/**
 * @import {
 *   ContractDecoder,
 *   LoggerConfig,
 *   Provider,
 *   ProviderConfig,
 *   SubscriptionConfig,
 *   SubscriptionEvent,
 * } from "./binding"
 */

/**
 * Each provider's subscription callback, held only as long as the provider is
 * reachable and never reached from the threadsafe function's root.
 *
 * @type {WeakMap<Provider, (event: SubscriptionEvent) => void>}
 */
const subscriptionCallbacks = new WeakMap();

class EdrContext extends binding.EdrContext {
  /**
   * @override
   * @param {string} chainType
   * @param {ProviderConfig} providerConfig
   * @param {LoggerConfig} loggerConfig
   * @param {SubscriptionConfig} subscriptionConfig
   * @param {ContractDecoder} contractDecoder
   * @returns {Promise<Provider>}
   */
  async createProvider(
    chainType,
    providerConfig,
    loggerConfig,
    subscriptionConfig,
    contractDecoder
  ) {
    // Populated once the provider exists (below); the trampoline reaches it
    // through this weak reference rather than capturing it.
    /** @type {{ provider?: WeakRef<Provider> }} */
    const weak = {};

    // The binding only ever sees the trampoline below, so its own check that
    // the callback is callable no longer covers the consumer's value.
    const userCallback = subscriptionConfig.subscriptionCallback;
    if (typeof userCallback !== "function") {
      throw new TypeError(
        `subscriptionConfig.subscriptionCallback must be a function, got ${typeof userCallback}`
      );
    }

    const provider = await super.createProvider(
      chainType,
      providerConfig,
      loggerConfig,
      {
        ...subscriptionConfig,
        subscriptionCallback: (event) => {
          const target = weak.provider?.deref();
          if (target !== undefined) {
            subscriptionCallbacks.get(target)?.(event);
          }
        },
      },
      contractDecoder
    );

    subscriptionCallbacks.set(provider, userCallback);
    weak.provider = new WeakRef(provider);

    return provider;
  }
}

// The spread's `EdrContext` (the base) is overwritten by the subclass that
// follows, so consumers see exactly one `EdrContext` — the guarded one. The
// override keeps `createProvider`'s signature, so `index.d.ts`'s
// `export * from "./binding"` describes it exactly.
//
// Spread `require("./binding")` directly rather than `binding`: Node derives a
// CommonJS module's ESM named exports by statically scanning it, and only
// recognizes a spread of a `require()` call as a re-export. Spreading the
// variable would leave ESM consumers with `EdrContext` as the sole named export.
module.exports = { ...require("./binding"), EdrContext };
