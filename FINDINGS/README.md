# FINDINGS — `hardhat test solidity -vvv` memory fix

Verification of the per-suite `mi_collect` fix for the solidity-test step-trace RSS balloon, with a memory-over-time analysis of whether the fix produces a real, net reduction.

---

## TL;DR

1. **The fix works and _nets_ a reduction.** Over 64 arena alloc/drop cycles (16 suites × 4 loops in one process), the **unfixed** binary climbs to a high plateau and **stays** there; the **fixed** binary saw-tooths (each suite's `mi_collect` returns pages) and ends **~60% lower**.
2. **It's jagged** — decreasing then increasing per suite — **but the envelope is bounded and low**, whereas unfixed keeps sitting at the high-water mark. More runtime makes the _unfixed_ plateau _higher_ (656 MB over 4 loops vs 519 MB over 1); the fixed envelope stays put.
3. **Scope (important):** this fix lives in the **solidity-test runner** only. The `eth_sendTransaction`-style allocation of a long-running node is the **provider / JSON-RPC path**, which this fix does **not** touch — so the analysis below answers the solidity-test case, not a node serving RPC. See "long-running allocation" below.
4. **Lossless**: identical test pass/fail with and without the fix; traces intact; no wall-clock regression.

---

## Analysis

### Does the fix actually reduce memory?

**Yes, for the solidity-test path — and it's a net reduction, not a wash.** The memory-over-time graph makes this concrete. Final (settled) RSS after 64 cycles:

| workload                  | unfixed final | fixed final | reduction |
| ------------------------- | ------------: | ----------: | --------: |
| `-vvv` (Always / Failing) |    **656 MB** |  **268 MB** |  **−59%** |
| `-vvvv` (Always / All)    |    **875 MB** |  **677 MB** |  **−23%** |

### Is it a real reduction, or does it just get jagged and trend back up?

A reasonable worry: since the program keeps allocating and freeing large arenas, freeing-then-reallocating might still leave RSS high, so the graph would just look "more jagged over time (decreasing before increasing)" at the same envelope.

**It _is_ jagged — but it does not trend back to the unfixed envelope.** The fixed line drops after each suite's `mi_collect`, then rises as the next suite allocates; its troughs return toward baseline every suite, so the running footprint stays bounded. The unfixed line, by contrast, reaches its high-water mark and **holds it for the rest of the run** (mimalloc retains freed pages by default). The graph shows the two behaviors side by side.

### What about long-running allocation (e.g. `eth_sendTransaction` / provider requests)?

Two separate things:

- **Solidity-test arenas (what this fix addresses):** confirmed bounded by the fix — see the graph.
- **`eth_sendTransaction` / provider arenas:** that is the **provider (JSON-RPC) code path**, which does _not_ flow through the solidity-test runner callback where `mi_collect` was added — so **this fix has no effect there**. (The fix's own "Why not…" rationale makes the same point about the provider `Response` path.) If provider-side RSS growth is a concern, it needs its own measurement and, likely, its own reclaim point — out of scope here, but worth a follow-up.

### Should `mi_collect` run only when the `CallTraceArena` is dropped?

**The data supports gating it.** At `-vvvv`/`-vvvvv` (`includeTraces=All`) the arenas are kept **live** (handed to JS), so they aren't freed at suite end and `mi_collect` can't reclaim them — which is exactly why the fix helps far less there (−23% vs −59%). Gating the collect on "an arena was actually dropped this suite" would (a) skip pointless collects when nothing is reclaimable and (b) keep the win where it exists. A reasonable refinement to fold in.

### Memory over time

Delivered: [`memory-over-time.html`](./memory-over-time.html). RSS is sampled every 20 ms across the whole process tree; the page plots the full trajectory for fixed vs unfixed at both `-vvv` and `-vvvv`, with loop boundaries marked.

---

## A/B verification (single run, 3 trials each)

Sustained RSS = `process.memoryUsage().rss` right after the run; matches the external process-tree engine (peak cross-validated: 532/516 ≈ 519/518).

| verbosity | config | unfixed sustained | fixed sustained | reduction |
| --- | --- | --: | --: | --: |
| `-vv` (default) | OnFailure / None | ~177 MB | ~177 MB | ~0 (nothing to reclaim ✔) |
| `-vvv` | Always / Failing | ~480–519 MB | ~208–218 MB | **~58%** |
| `-vvvv` / `-vvvvv` | Always / All | ~404–544 MB | ~315–326 MB | **~40%** |

- **Mechanism confirmed:** `MIMALLOC_PURGE_DELAY=0` on the _unfixed_ binary collapses `-vvv` sustained 519 → 196 MB; the fix lands at that same floor — proving the balloon is retained pages, not a leak.
- **Peak RSS is ~unchanged by the fix** (~518 both at `-vvv`): the fix lowers the _sustained/settled_ footprint, not the momentary peak while traces are recorded.
