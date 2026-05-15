# `Provider` lifecycle relies on JS GC, causing TSFN UAF crashes at process exit

## Summary

EDR's napi `Provider` exposes no way to release the underlying threadsafe-functions (subscription, call-override, logger, coverage / gas-report aggregators) explicitly. Consumers depend on Node's natural cleanup — JS GC of the wrapper, then `napi_finalize`, then Rust `Drop` — to release them.

When `napi_finalize` fires _during Node `Environment::CleanupHandles`_ (which it does whenever GC didn't reclaim the wrapper before the process started exiting), it races [a long-known data race / use-after-free in `napi_threadsafe_function`](https://github.com/nodejs/node/issues/55706). The Node-side fix landed in [PR #55877](https://github.com/nodejs/node/commit/350b0ea895) (commit `350b0ea895`) on 2026-01-13 and is in **Node 25 only** — no backport to Node 22 or 20 LTS.

Symptom: process exits with SIGABRT / SIGBUS / SIGSEGV _after_ mocha (or any test framework) reports passing. Also surfaces as deadlock-class hangs (CI burning to the timeout cap), or "script never ends" reports (the inverse symptom — atexit hooks don't drain).

The bug has been hitting EDR users in the wild since at least 2024-08 (see [#590](https://github.com/NomicFoundation/edr/issues/590), reproducible Linux + Hardhat + interval mining). PR [#1378](https://github.com/NomicFoundation/edr/pull/1378) (merged 2026-05-05) addressed one specific manifestation (interval-mining task accumulation deadlocking CI on Windows / macOS) via a special-case `evm_setIntervalMining(0)` afterEach. The general fix — a `Provider.close()` method that drops the underlying `Arc<dyn SyncProvider>` cascade synchronously, in the JS event-loop window before atexit — is what this issue tracks.

## Symptom: matching evidence across the issue tracker

[#590](https://github.com/NomicFoundation/edr/issues/590) is the diagnostic case. User-reported, deterministic, Linux + Hardhat 2 + `node:test` + interval mining. Stack trace (verbatim):

```
thread '<unnamed>' panicked at /build/crates/edr_provider/src/interval.rs:92:18:
Failed to send cancellation signal: ()
stack backtrace:
   ...
  16:           0xf15f6a - node_napi_env__::CallFinalizer
  17:           0xeefddb - v8impl::Reference::Finalize
  18:           0xf1a872 - v8impl::ThreadSafeFunction::CloseHandlesAndMaybeDelete
  19:          0x1e34661 - uv__finish_close
                                at deps/uv/src/unix/core.c:351:5
  20:          0x1e34661 - uv__run_closing_handles
  21:          0x1e34661 - uv_run
  22:           0xeca0a0 - node::Environment::CleanupHandles
  23:           0xeca15c - node::Environment::RunCleanup
  24:           0xe698d1 - node::FreeEnvironment
  25:           0xfc0c80 - node::NodeMainInstance::Run
   ...
fatal runtime error: failed to initiate panic, error 5
... signal: 'SIGABRT'
```

Reading the frames bottom-up: Node is in `Environment::CleanupHandles` / `FreeEnvironment` (env-teardown phase) → libuv runs closing handles via `uv_run` → that fires `ThreadSafeFunction::CloseHandlesAndMaybeDelete` and `Reference::Finalize` → finalize calls back into our Rust drop chain → the chain reaches `IntervalMiner::Drop` at `interval.rs:92:18` and panics on `cancellation_sender.send(()).expect(...)`. The send fails because the receiver has already been dropped — the tokio runtime has already begun shutting down by the time finalize runs.

This is the textbook napi TSFN env-cleanup UAF/race documented in [nodejs/node#55706](https://github.com/nodejs/node/issues/55706). Reporter: _"This is not a problem on Mac OS setups."_ Reporter worked around by switching CI to macOS.

Related issues (same root cause, different surface):

- [#771](https://github.com/NomicFoundation/edr/issues/771) "Multichain EDR crashes if interval mining is non-zero" (2025-01-16, closed). Same trigger, internally filed.
- [#385](https://github.com/NomicFoundation/edr/issues/385) "vitest watch mode crash with EDR" (2024-04-14, closed). Multi-provider lifecycle on vitest reload; "Aborted (core dumped)" + `failed to initiate panic, error 5`.
- [NomicFoundation/hardhat#4997](https://github.com/NomicFoundation/hardhat/issues/4997) "Run a script using the hardhat network, makes the script to never end" (2024-03-14, closed). Inverse symptom — process hangs at exit instead of crashing. Same lifecycle gap.

The cumulative pattern: **the napi env-cleanup race has been hitting EDR users for at least 18 months.** Most reports were filed without the keywords (SIGBUS / SIGSEGV / threadsafe / atexit) that would've connected them to the upstream Node issue. Several were closed without a true root-cause fix.

A second instance was discovered while developing the napi-rs v3 migration ([#1385](https://github.com/NomicFoundation/edr/pull/1385)) — `Test EDR TS bindings (macos-15)` job crashed deterministically with two distinct signals across two runs:

| Run | Signal       | Exit code | Stage                                  |
| --- | ------------ | --------- | -------------------------------------- |
| 1   | SIGBUS (10)  | 138       | After mocha printed `46 passing (12s)` |
| 2   | SIGSEGV (11) | 139       | After mocha printed `46 passing (11s)` |

Two distinct signals from the same code = "stale pointer access during atexit, lands in different page mappings each run." Cross-OS: this PR's symptom is **macOS-15 only, Linux works**; #590's is **Linux only, macOS works**. Same race, opposite platform expression.

## Root cause

The race window during process exit:

1. Node fires environment cleanup hooks (LIFO order).
2. napi-rs registers a hook that calls `Runtime::shutdown_background()` on its tokio runtime ([`crates/napi/src/tokio_runtime.rs`](https://github.com/napi-rs/napi-rs/blob/main/crates/napi/src/tokio_runtime.rs)) — **non-blocking**, returns immediately even with in-flight tasks.
3. Concurrently, TSFN finalize fires; `napi_release_threadsafe_function(release)` runs as the `Arc<ThreadsafeFunctionHandle>` drops.
4. A late tokio task — still running on a worker thread — calls into the now-released TSFN's freed context. UAF.
5. The corrupt pointer lands wherever the allocator currently maps free pages → SIGBUS / SIGSEGV / SIGABRT depending on platform allocator state.

Mika Fischer's own assessment of working around this from the consumer side (in [#55706](https://github.com/nodejs/node/issues/55706)): _"very unergonomic."_ The recommended pattern requires an external finalizer tracking finalization state, `shared_ptr` for lifetime management, and a mutex protecting all TSFN access.

The Node-side fix landed in [#55877](https://github.com/nodejs/node/commit/350b0ea895) — rewrites `napi_threadsafe_function` with a 3-state enum (`kOpen` / `kClosing` / `kClosed`), mutex-protected finalization, deferred deletion until `thread_count == 0`. **Node 25 only**; not backported to Node 22 or 20. Node 20 is past LTS EOL.

### napi-rs v3 made it slightly worse for consumers

v3 dropped the explicit `.unref(env)` API in favor of the type-level `weak::<true>()`. That removed a workaround surface v2 consumers had used (manual `.unref(env)` + explicit cleanup at known-safe times). Net: v3 has nicer ergonomics but no compensating mechanism to release TSFNs synchronously before env cleanup.

Background context across the napi-rs issue tracker (TSFN cleanup has a long history of bugs): [napi-rs#1220](https://github.com/napi-rs/napi-rs/issues/1220), [#272](https://github.com/napi-rs/napi-rs/issues/272), [#518](https://github.com/napi-rs/napi-rs/issues/518), [#612](https://github.com/napi-rs/napi-rs/issues/612), [#2460](https://github.com/napi-rs/napi-rs/issues/2460) (macOS-specific regression), [#3251](https://github.com/napi-rs/napi-rs/issues/3251) (open).

## Why macOS-15 surfaces it (when it does)

It's _not_ a Sequoia regression. Apple security release notes 15.0–15.6, dyld release notes, and pthread cleanup docs surfaced no relevant change. The same `.node` runs clean on the same `macos-15` runner under a different test workload (`Run Hardhat tests (macos-15, Node 20)`) — strongest evidence that the binary itself is fine; only specific test workloads trigger the crash.

What macOS-15 _does_ differ in is its allocator state at the moment napi-rs's race happens. The freed page that gets accessed is, on macOS 15, sometimes unmapped due to `madvise`/`MADV_FREE` aggressiveness → SIGBUS / SIGSEGV. Linux + Windows have different allocator behaviour, so the same race lands on a still-mapped (garbage) page and the corruption goes silently — until it crashes elsewhere or causes #590's deterministic Linux panic.

## Real-world user impact

A `Provider` instance in EDR holds:

- One TSFN per logger callback (decode-console-log, print-line)
- One TSFN per `set_call_override_callback`
- One TSFN for the subscription callback
- One TSFN per coverage / gas-report callback
- An `IntervalMiner` background task (when interval mining is configured)

**The race surface scales with TSFN count × provider count.** Patterns that hit it:

| Pattern | Risk | Notes |
| --- | --- | --- |
| `hardhat test` (single provider) | Low | One provider, short cleanup window. |
| `hardhat node` (long-lived, killed via SIGINT) | Low | Doesn't reach natural atexit. |
| `hardhat run script.js` with one provider | Low | Same as `hardhat test`. |
| **Programmatic multi-provider scripts** | **High** | Each provider adds shutdown-window probability. |
| Test runners looping over chain configs | High | Common in benchmark suites, integration tests, fork comparison tools. |
| `eth_subscribe`-heavy patterns without unsubscribe | High | Long-lived subscription TSFNs are exactly the canonical "in-flight async work at finalize" UAF trigger. |

Hardhat itself partially shields _typical users_ because Hardhat's task lifecycle drops/dereferences providers during teardown. That's why HH2's `Run Hardhat tests` workflow on `macos-15` is clean on the same `.node` while EDR's mocha test set crashes — Hardhat's tests use Hardhat's own teardown machinery; EDR's tests don't.

**Programmatic users — anyone using EDR's napi binding outside Hardhat's task scope, or creating multiple Hardhat networks in one process — don't get this protection.**

## Workarounds, with trade-offs

| Workaround | What it does | Verdict |
| --- | --- | --- |
| **`mocha --exit`** | Forces `process.exit(code)` after results. Short-circuits env cleanup hooks. | Band-aid. Converts UAF into a different UAF surface ([nodejs/node-addon-api#591](https://github.com/nodejs/node-addon-api/issues/591) "Fatal error when using N-API async code from process exit hooks"). Don't recommend. |
| **Drop JS reference + trust GC (HH2 pattern)** | `delete this.provider` in `afterEach`, rely on GC firing finalize before atexit. Empirically reliable for typical Hardhat workloads, fragile for short scripts / multi-provider patterns. | Hardhat-only; doesn't help library consumers. |
| **Special-case RPC teardown** ([#1378](https://github.com/NomicFoundation/edr/pull/1378)) | `evm_setIntervalMining(0)` in `afterEach` to drop the IntervalMiner's task synchronously. | Addresses one specific resource (interval mining). Doesn't address TSFNs. Requires consumer wiring per resource type. |
| **`Provider.close()`** (proposed) | Drops the inner `Arc<dyn SyncProvider>` synchronously inside the JS event loop, triggering the full cleanup cascade — IntervalMiner cancel/join, ProviderData drop, TSFN release — before env cleanup runs. | The general fix. |

## Proposed fix: `Provider.close()`

Add an async `close()` method on `Provider` that:

1. Takes the inner `Arc<dyn SyncProvider>` out of a new `Mutex<Option<Arc<...>>>` field.
2. Drops the Arc inside `tokio::runtime::spawn_blocking`, ensuring the cleanup cascade runs on a tokio worker thread (`IntervalMiner::Drop` calls `block_in_place` + `block_on(background_task)` to cancel and join the interval-mining task; that requires a tokio context).
3. Awaits the resulting `JoinHandle` so the JS-side `await provider.close()` resolves only after the cleanup cascade has fully completed.

After close, all subsequent method calls return `napi::Error::from_reason("Provider has been closed")`. Calling close twice is idempotent.

The cleanup cascade (when the `Arc<dyn SyncProvider>` count reaches 0):

```
Arc<dyn SyncProvider> → 0 refs → edr_provider::Provider::Drop
  → interval_miner field drops → IntervalMiner::Drop
    → cancellation_sender.send(()) → background_task returns
    → Arc<AsyncMutex<ProviderData>> count → 0 → ProviderData::Drop
      → logger / subscriber_callback / call_override_callback drop
        → Arc<ThreadsafeFunctionHandle>s drop
          → napi_release_threadsafe_function (env still healthy)
```

This runs synchronously in the JS event-loop window — _before_ Node enters `Environment::CleanupHandles`. The `cancellation_sender.send()` that panics in #590 succeeds here because the runtime is still healthy.

### TC39 `Symbol.asyncDispose`

Not added directly to the napi class — napi-rs v3's `js_name` attribute creates a string-named property, not a Symbol-keyed one, so `await using` doesn't trigger it. JS-side wrappers (Hardhat 3's `EdrProvider`, etc.) can add `[Symbol.asyncDispose] = function() { return this.close() }` for `await using` ergonomics if desired.

### UX layering

| Layer | Audience | Surface |
| --- | --- | --- |
| 1 | Normal Hardhat user | Invisible. Never calls `close()`. Hardhat handles it. |
| 2 | Hardhat itself | Calls `provider.close()` in network manager / task teardown / signal handler. ~5–10 LOC of wiring per consumer (HH3 PR + HH2 patch). |
| 3 | Advanced / library users | `await using provider = ...` (modern, via JS-side wrapper) or `try { ... } finally { await provider.close() }` (fallback). |

### Safety policy

| Scenario | Behavior |
| --- | --- |
| `close()` not called, normal Drop fires at process exit | Best-effort, same risk profile as today (the napi-rs Drop still runs `release`; UAF window remains). |
| `close()` called once, then Drop fires | Idempotent — `Mutex<Option<Arc<...>>>` is already `None`, nothing to drop. |
| `close()` called twice | No-op the second time. |
| Method called after `close()` | Returns `napi::Error::from_reason("Provider has been closed")`. |

Floor stays at today's behavior; ceiling rises to "guaranteed safe if `close()` is called at the right time."

## What this fix subsumes

- **#590** — `IntervalMiner::Drop` panic at `interval.rs:92:18` no longer fires because the Drop runs while the runtime is healthy.
- **#771** — same trigger, fixed via the same mechanism.
- **#385** — multi-provider vitest reload: each provider closed before the next runs.
- **#1378's `evm_setIntervalMining(0)` afterEach pattern** — becomes a special case of `close()`'s general drop cascade. The afterEach in `test/logs.ts` simplifies to `await provider.close()`.

## What it doesn't address

- The race surface still exists for consumers who don't call `close()`. Users who construct providers outside of Hardhat and forget the explicit close still hit the original probability of crash. **Fundamental fix is upstream Node #55877 reaching LTS.** Until then, `close()` shifts the burden from "every consumer is implicitly racing" to "every consumer can opt in to safety with one method call." Hardhat 3 and Hardhat 2 wire it into their task lifecycles so end users don't have to.

## References

- [`nodejs/node#55706`](https://github.com/nodejs/node/issues/55706) — "napi_threadsafe_function is very hard to use safely" (Mika Fischer, 2024-11-03)
- [`nodejs/node#55877`](https://github.com/nodejs/node/commit/350b0ea895) — Node-side fix; **Node 25 only**, no LTS backport
- [`napi-rs/napi-rs#1220`](https://github.com/napi-rs/napi-rs/issues/1220) — Memory and safety issues with ThreadsafeFunction (Prisma)
- [`NomicFoundation/edr#590`](https://github.com/NomicFoundation/edr/issues/590) — diagnostic case (open since 2024-08)
- [`NomicFoundation/edr#771`](https://github.com/NomicFoundation/edr/issues/771) — internal multichain crash (closed 2025-02)
- [`NomicFoundation/edr#385`](https://github.com/NomicFoundation/edr/issues/385) — vitest watch crash (closed v0.3.6)
- [`NomicFoundation/edr#1378`](https://github.com/NomicFoundation/edr/pull/1378) — IntervalMiner-specific workaround (merged 2026-05-05)
- [`NomicFoundation/edr#1385`](https://github.com/NomicFoundation/edr/pull/1385) — napi-rs v3 migration; `[BISECT]` skip on macos-15 surfaced this pattern again
- [`NomicFoundation/hardhat#4997`](https://github.com/NomicFoundation/hardhat/issues/4997) — "script never ends" inverse symptom (closed)

Full investigation notes (root cause, alternatives considered, scoping notes) at `/workspace/napi-rs-v3-tsfn-shutdown-investigation.md` in the EDR working tree, will be moved into the issue/PR cross-reference once filed.
