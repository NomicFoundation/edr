---
"@nomicfoundation/edr": minor
---

- Added `L1Hardfork` enum, with all post-Byzantium L1 hardforks. Discriminants of the variants match those of `SpecId`.
- Fixed an issue for forked blockchains where a block fork block that precedes its oldest supported hardfork was silently accepted. Now it fails with an error naming the oldest supported hardfork.
- BREAKING CHANGE: Removed `SpecId`. Instead, use `L1Hardfork`. Using any of the pre-Byzantium hardforks (`Frontier`, `FrontierThawing`, `Homestead`, `DaoFork`, `Tangerine` and `SpuriousDragon`) previously resulted in a runtime error. Now, they are no longer representable.
