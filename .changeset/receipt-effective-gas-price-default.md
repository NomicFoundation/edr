---
"@nomicfoundation/edr": patch
---

Fixed `eth_getTransactionReceipt` and `eth_getTransactionByHash` failing for remote transactions when forking from a node that omits `effectiveGasPrice` from receipts, such as a Hardhat node running a pre-London hardfork.
