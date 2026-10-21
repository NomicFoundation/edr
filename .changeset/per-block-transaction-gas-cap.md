---
"@nomicfoundation/edr": patch
---

Fixed calls and gas estimates at remote blocks of a forked blockchain being checked against the provider's transaction gas cap. Remote blocks are the fork block and the blocks before it. Those requests now use the cap of the remote chain's hardfork at that block. For example, an Osaka provider no longer rejects an explicit `gas` above `2^24` at a pre-Osaka remote block. A Prague provider now enforces that cap at an Osaka remote block. Replays by `debug_traceCall` and `debug_traceTransaction` at remote blocks also use the gas cap of their block.
