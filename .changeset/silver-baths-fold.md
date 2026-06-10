---
"@nomicfoundation/edr": minor
---

- Migrated `edr_napi` to napi-rs v3.
- The `reason`, `counterexample`, and `valueSnapshotGroups` fields on `TestResult` are now class getters returning `T | undefined`.
