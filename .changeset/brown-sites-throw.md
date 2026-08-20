---
"@nomicfoundation/edr": minor
---

Added native inline configuration parsing in Solidity tests. Ill-formed inline configuration rejects the whole run up front, reporting one problem per affected test function; the rejected `runSolidityTests` promise carries the structured, located problems as the `inlineConfigErrors` array on the thrown error.

Removed inline config associated types from NAPI: `TestFunctionOverride`, `TestFunctionIdentifier`, `TestFunctionConfigOverride`, `FuzzConfigOverride`, `InvariantConfigOverride`, and `TimeoutConfig`. Removed the `testFunctionOverrides` field of `SolidityTestRunnerConfigArgs`.
