---
"@nomicfoundation/edr": minor
---

Renamed the hardfork name strings to match Hardhat's definitions: L1 names are now camelCase (e.g. `"byzantium"`, `"muirGlacier"`, `"arrowGlacier"`) and OP names lowercase (e.g. `"bedrock"`, `"isthmus"`). `l1HardforkToString`/`opHardforkToString` return the new names, `l1HardforkFromString`/`opHardforkFromString` and provider configs accept only them, and the exported string constants (`BYZANTIUM`, …, `AMSTERDAM` and `BEDROCK`, …, `INTEROP`) now hold the new values.

Passing an old-style name (e.g. `"Byzantium"`, `"Arrow Glacier"`) now throws `The provided hardfork \`<name>\` is not supported.`
