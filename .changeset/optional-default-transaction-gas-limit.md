---
"@nomicfoundation/edr": minor
---

Changed `defaultTransactionGasLimit` in the provider config to be optional. Hosts that pass it are unaffected.

When it is omitted, a request without `gas` uses the bound of its block. That bound is `2^32 - 1` from Amsterdam (EIP-8037), `2^24` from Osaka (EIP-7825), and the block gas limit before. The block gas limit caps each of these. Before Amsterdam, a configured `transactionGasCap` also bounds the derived value.

The transaction gas cap that EDR enforces for its configured hardfork bounds the value for earlier blocks too. For example, an Osaka provider gives `2^24` for a call at a Prague block.

Without a configured `transactionGasCap`, a transaction without `gas` gets the block gas limit before Osaka and from Amsterdam. From Amsterdam, `2^32 - 1` caps it. The same holds under Osaka with a disabled `transactionGasCap` or a block gas limit below `2^24`. With manual or interval mining, a block then fits fewer such transactions. Their senders also need a larger upfront balance.
