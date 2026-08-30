---
"@nomicfoundation/edr": minor
---

Implemented the `vm.broadcastRawTransaction(bytes)` cheatcode in the Solidity test runner, which previously failed as an unsupported cheatcode. The RLP-encoded signed transaction is decoded and executed against the current EVM state from the address recovered from its signature, matching Foundry's behaviour in a test context. This unblocks replaying pre-signed transactions in Solidity fixtures, in particular deterministic-deployment bootstraps that use Nick's method, whose factory address is bound to a specific keyless sender and so cannot be re-encoded as an ordinary call or pranked.

The other scripting cheatcodes (`broadcast`, `startBroadcast`, `stopBroadcast`, `getBroadcast`, and the wallet and deployment-artifact cheatcodes) remain unsupported.
