---
"@nomicfoundation/edr": patch
---

- Added the EIP-8037 gas dimensions to `debug_traceTransaction` and `debug_traceCall` responses from the Amsterdam hardfork: the default tracer's top-level frame and the `callTracer`'s root call frame gain `executionGasUsed`, `stateGasUsed` and `gasRefund`, and each `structLogs` entry gains `stateGasReservoir`, plus a signed `stateGasCost` on the steps that charge or refill state gas.
- Added the `stateGasTracer`, which reports a transaction's gas in both dimensions on every hardfork, with zero state gas before Amsterdam.
