---
"@nomicfoundation/edr": minor
---

Fixed Solidity stack traces after a precompile call. A call that follows a precompile call in the same function is now attributed to the right line. A failing precompile call now produces a `PRECOMPILE_ERROR` entry instead of an unrecognized-contract error. JSON-RPC call traces therefore include calls to precompiles, as Solidity test call traces already did.
