---
"@nomicfoundation/edr": minor
---

- Added a `getCoverageLibrary()` helper at the `@nomicfoundation/edr/coverage` subpath that returns the library's source and expected filename.
- Bundled the Solidity coverage library with EDR.
- Changed `addStatementCoverageInstrumentation` to no longer accept a library path argument.
