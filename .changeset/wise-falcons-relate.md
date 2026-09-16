---
"@nomicfoundation/edr": minor
---

- Changed the hardfork name strings to match Hardhat's definitions: names are now camelCase (e.g. `"byzantium"`, `"muirGlacier"`, `"bedrock"`). `l1HardforkToString`/`opHardforkToString` return the new names, and `l1HardforkFromString`/`opHardforkFromString` and provider configs accept only them — passing an old-style name (e.g. `"Byzantium"`) fails.
- BREAKING CHANGE: Removed hardfork name string constants (`BYZANTIUM`, …, `AMSTERDAM` and `BEDROCK`, …, `ISTHMUS`). Instead, obtain them using `l1HardforkToString` (e.g. replace `OSAKA` with `l1HardforkToString(L1Hardfork.Osaka)`).
