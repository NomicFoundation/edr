---
"@nomicfoundation/edr": minor
---

Removed support for pre-Byzantium Ethereum L1 hardforks. The `SpecId` enum no longer includes `Frontier`, `FrontierThawing`, `Homestead`, `DaoFork`, `Tangerine` and `SpuriousDragon`, and the corresponding `FRONTIER`, `FRONTIER_THAWING`, `HOMESTEAD`, `DAO_FORK`, `TANGERINE` and `SPURIOUS_DRAGON` string constants are gone. Discriminants of the remaining variants are unchanged, so `Byzantium` is still `6`.

Passing one of the removed hardfork names now throws `The provided hardfork \`<name>\` is not supported.`

Forking a chain from a block that precedes its oldest supported hardfork now fails with an error naming that hardfork, instead of silently skipping hardfork validation.
