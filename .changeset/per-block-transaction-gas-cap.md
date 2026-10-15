---
"@nomicfoundation/edr": patch
---

Fixed calls and gas estimates at a block of an older hardfork being checked against the provider's transaction gas cap. They now use the cap of that block's hardfork. For example, an Osaka provider no longer rejects an explicit `gas` above `2^24` at a pre-Osaka fork block. A Prague provider now enforces that cap at an Osaka fork block. `debug_traceCall` and `debug_traceTransaction` replays also use the bounds of their block.
