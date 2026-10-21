---
"@nomicfoundation/edr": minor
---

Changed `defaultTransactionGasLimit` in the provider config to be optional.

When it is omitted, a request without `gas` uses the bound of its block. That bound is `2^32 - 1` from Amsterdam (EIP-8037), `2^24` from Osaka (EIP-7825), and the block gas limit before. The block gas limit caps each of these. Before Amsterdam, a configured `transactionGasCap` replaces the hardfork's bound.

When the derived value is the block gas limit, a transaction without `gas` reserves the whole block. With manual or interval mining, a block then fits fewer such transactions. Their senders also need a larger upfront balance.
