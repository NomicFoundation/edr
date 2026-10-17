---
"@nomicfoundation/edr": minor
---

Changed provider creation to fail when `defaultTransactionGasLimit` exceeds the transaction gas bound of the configured hardfork and transaction gas cap. Before, every request without `gas` failed instead. A configured value is used as is. A request at a block whose bound is lower fails like a request with an explicit `gas` above that bound.
