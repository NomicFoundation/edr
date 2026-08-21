---
"@nomicfoundation/edr": minor
---

Renamed the `SpecId` enum to `L1Hardfork`, mirroring `OpHardfork`, and removed support for pre-Byzantium Ethereum L1 hardforks: the enum no longer includes `Frontier`, `FrontierThawing`, `Homestead`, `DaoFork`, `Tangerine` and `SpuriousDragon`. Discriminants of the remaining variants are unchanged, so `Byzantium` is still `6`.

Forking a chain from a block that precedes its oldest supported hardfork now fails with an error naming that hardfork, instead of silently skipping hardfork validation.
