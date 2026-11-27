---
"@nomicfoundation/edr": patch
---

Fixed Solidity tests that lower the hardfork below Amsterdam with an inline `evm_version` failing with a transaction gas cap error when `transactionGasCap` is set. The default gas limit and the transaction gas cap now follow the overridden hardfork, while a configured `gasLimit` or `blockGasLimit` is still used as is.
