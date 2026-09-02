---
"@nomicfoundation/edr": patch
---

Fixed calls through proxy contracts being reported as `<unrecognized-selector>` in logged transactions and calls. When a function selector is not found in the called contract's ABI, it is now resolved against the ABI of the implementation behind the proxy, and the call is labeled with the full delegation chain, e.g. `Proxy>Implementation#setValue`. This works for any proxy that forwards the selector via `DELEGATECALL`.
