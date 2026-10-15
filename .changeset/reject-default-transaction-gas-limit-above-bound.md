---
"@nomicfoundation/edr": minor
---

Changed provider creation to fail when `defaultTransactionGasLimit` exceeds the transaction gas bound of the configured hardfork. Before, every request without `gas` failed instead.
