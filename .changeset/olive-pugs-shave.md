---
"@nomicfoundation/edr": minor
---

Added profile support to Solidity test inline configuration. `SolidityTestRunnerConfigArgs` now accepts `testProfile` and `declaredTestProfiles`. Unprefixed directives apply to every profile, while profile-prefixed directives apply only when that profile is selected and override unprefixed directives with the same key. Undeclared profile prefixes are rejected.

BREAKING CHANGE: Renamed the `InlineConfigUnsupportedProfile` inline config problem to `InlineConfigUndeclaredProfile`. The replacement also includes the declared profile names in `declaredProfiles`.
