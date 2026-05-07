---
"@nomicfoundation/edr": minor
---

- Migrated `edr_napi` to napi-rs v3.
- The `MineOrdering`, `TestStatus`, and `CheatcodeErrorCode` enums are now type unions instead of runtime enums; consumers must use string literals at value positions.
- The `reason`, `counterexample`, and `valueSnapshotGroups` fields on `TestResult` are now class getters returning `T | undefined`.
