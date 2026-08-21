---
"@nomicfoundation/edr": minor
---

Renamed the hardfork name strings to match Hardhat's definitions: names are now camelCase (e.g. `"byzantium"`, `"muirGlacier"`, `"bedrock"`). `l1HardforkToString`/`opHardforkToString` return the new names, and `l1HardforkFromString`/`opHardforkFromString` and provider configs accept only them — passing an old-style name (e.g. `"Byzantium"`) fails.

The exported hardfork name string constants (`BYZANTIUM`, …, `AMSTERDAM` and `BEDROCK`, …, `ISTHMUS`) were removed; convert from the enum instead, e.g. replace `OSAKA` with `l1HardforkToString(SpecId.Osaka)`. (Note that `SpecId` is a numeric enum, so `SpecId.Osaka.toString()` yields `"19"`, not the name.)
