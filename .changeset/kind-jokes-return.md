---
"@nomicfoundation/edr": minor
---

Removed support for the L1 hardforks that revm removed as EVM-equivalent: Frontier Thawing, DAO Fork, Constantinople, Muir Glacier, Arrow Glacier, and Gray Glacier. Their `SpecId` variants and name constants are gone, and provider configurations using these hardfork names are rejected; use the EVM-equivalent surviving hardfork variants instead (Frontier, Homestead, Petersburg, Istanbul, or London).
