---
"@nomicfoundation/edr": minor
---

Added support for the `vm.broadcastRawTransaction(bytes)` cheatcode in Solidity tests. The RLP-encoded signed transaction is decoded and executed against the current EVM state from the address recovered from its signature, matching Foundry's behavior in a test context. This makes it possible to replay pre-signed transactions such as deterministic-deployment bootstraps (Nick's method), which cannot be reproduced with `vm.prank`.
