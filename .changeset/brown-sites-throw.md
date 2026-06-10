---
"@nomicfoundation/edr": minor
---

Added native inline configuration parsing in Solidity tests.

Removed inline config associated types from NAPI: `TestFunctionOverride`, `TestFunctionIdentifier`, `TestFunctionConfigOverride`, `FuzzConfigOverride`, `InvariantConfigOverride`, and `TimeoutConfig`. Removed the `testFunctionOverrides` field of `SolidityTestRunnerConfigArgs`.
