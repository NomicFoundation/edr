---
"@nomicfoundation/edr": minor
---

The `eip712HashType` and `eip712HashStruct` cheatcodes now resolve type names by parsing the running test contract's Solidity sources, using the absolute paths supplied via the `testSourcePaths` runner config (shared with inline configuration parsing) and resolving non-relative imports through `importMappings`. Removed the `eip712CanonicalTypes` field of `SolidityTestRunnerConfigArgs`.

Each test source is read and parsed once per run, at runner creation, serving both inline configuration and EIP-712 type collection. An empty (or omitted) `testSourcePaths` disables collection — name lookups by the EIP-712 cheatcodes then report an unknown type, while inline definitions still work. A non-empty map must cover every test suite whose source can be parsed (solc >= 0.8): a missing entry, an unreadable or unparseable source, or an unsupported solc version for a listed source rejects the whole run up front, reported via the structured `inlineConfigErrors` array on the thrown error (new entries: `InlineConfigSourcePathNotProvided`, `InlineConfigSourceParseError`).
