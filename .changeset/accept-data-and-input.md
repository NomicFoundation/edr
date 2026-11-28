---
"@nomicfoundation/edr": patch
---

Fixed `eth_call`, `eth_estimateGas`, `eth_sendTransaction` and `debug_traceCall` rejecting transaction objects that set both `data` and `input`, as Foundry's `cast` does. As in geth, calls use `input` when the two differ, and `eth_sendTransaction` rejects different values as invalid params.
