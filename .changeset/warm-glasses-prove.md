---
"@nomicfoundation/edr": minor
---

Added verbose tracing for hardhat-tracer. Breaking change: The `stack_top`
property of `edr_napi::trace::TracingStep` was removed and `stack` was added
instead. Please see the documentation of the struct for details.
