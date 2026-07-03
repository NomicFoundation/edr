---
"@nomicfoundation/edr": minor
---

- Changed the `reason`, `counterexample`, and `valueSnapshotGroups` fields on `TestResult` to class getters returning `T | undefined`.
- Changed `SuiteResult` from a class to a plain object; field shapes are unchanged.
- Fixed exceptions thrown by the `decodeConsoleLogInputsCallback` and `printLineCallback` logger callbacks from being swallowed or crashing the process. They now surface as JSON-RPC internal-error responses carrying the JS error message.
