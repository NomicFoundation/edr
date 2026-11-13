---
"@nomicfoundation/edr": patch
---

Fixed blocks mined on top of a fork using default blob and base fee parameters instead of the forked chain's when the configured `chainId` differs from the remote chain's.
