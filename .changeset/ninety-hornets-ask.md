---
"@nomicfoundation/edr": minor
---

Removed `getLatestSupportedSolcVersion` API

BREAKING CHANGE: A new API `latestSupportedSolidityVersion` was previously introduced to replace the deprecated `getLatestSupportedSolcVersion`. The old API has now been removed. Users should update their code to use `latestSupportedSolidityVersion` instead.
