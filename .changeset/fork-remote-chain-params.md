---
"@nomicfoundation/edr": patch
---

Fixed blocks mined on top of a fork not following the forked chain's protocol parameters (BPO blob schedule and base fee parameters) when the configured `chainId` differs from the remote chain's, e.g. the default `31337`. These parameters are now looked up by the remote chain ID; `chainId` only sets the local chain's identity. An explicit `baseFeeParams` config still takes precedence.
