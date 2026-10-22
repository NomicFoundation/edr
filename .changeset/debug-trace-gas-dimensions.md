---
"@nomicfoundation/edr": patch
---

Added the EIP-8037 gas dimensions to `debug_traceTransaction` and `debug_traceCall` responses from the Amsterdam hardfork: the default tracer's top-level frame gains `executionGasUsed`, `stateGasUsed` and `gasRefund`, each of its `structLogs` entries gains `stateGasCost` and `stateGasReservoir`, and the `callTracer`'s root call frame gains `executionGasUsed`, `stateGasUsed` and `gasRefund`.
