---
"@nomicfoundation/edr": patch
---

Fixed `hardhat_setStorageAt` dropping storage that was set on the same account in earlier blocks when the state of a block is rebuilt instead of read from the state cache.
