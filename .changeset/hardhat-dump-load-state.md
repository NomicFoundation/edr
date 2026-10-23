---
"@nomicfoundation/edr": minor
---

Added the `hardhat_dumpState` and `hardhat_loadState` JSON-RPC methods. `hardhat_dumpState` returns the balance, nonce, code, and storage of every account as gzip-compressed JSON, in the same format as the accounts of Anvil's state dumps. `hardhat_loadState` merges such a dump, from Hardhat or from Anvil, into the current state. Dumping isn't supported in fork mode yet.
