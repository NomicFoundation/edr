---
"@nomicfoundation/edr": patch
---

Fixed the state at a block inside a range mined by `hardhat_mine`, which was empty once the block had been read: balances, nonces, code, and storage were zero.
