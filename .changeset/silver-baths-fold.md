---
"@nomicfoundation/edr": minor
---

- Migrated `edr_napi` to napi-rs v3.
- The `reason`, `counterexample`, and `valueSnapshotGroups` fields on `TestResult` are now class getters returning `T | undefined`.
- `SuiteResult` is now a plain object instead of a class; field shapes are unchanged.
- Exceptions thrown by the `decodeConsoleLogInputsCallback` and `printLineCallback` logger callbacks now surface as JSON-RPC internal-error responses carrying the JS error message, instead of being swallowed or crashing the process.
