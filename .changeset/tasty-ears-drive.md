---
"@nomicfoundation/edr": minor
---

Added support to the `debug_traceCall` & `debug_traceTransaction` JSON-RPC methods for different tracers (`4byteTracer`, `callTracer`, `flatCallTracer`, `prestateTracer`, `noopTracer`, and `muxTracer`).

Our API is now aligned with Geth's tracing capabilities.

BREAKING CHANGE: Memory capture used to be enabled by default on geth, but has since been flipped <https://github.com/ethereum/go-ethereum/pull/23558> and is now disabled by default. We have followed suit and disabled it by default as well. If you were relying on memory capture, you will need to explicitly enable it by setting the `enableMemory` option to `true` in your tracer configuration.
