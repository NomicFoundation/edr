---
"@nomicfoundation/edr": minor
---

The `eip712HashType` and `eip712HashStruct` cheatcodes now resolve type names by parsing the running test contract's Solidity sources, using the absolute paths supplied via the `testSourcePaths` runner config (shared with inline configuration parsing) and resolving non-relative imports through `importMappings`. Removed the `eip712CanonicalTypes` field of `SolidityTestRunnerConfigArgs`; a suite without a `testSourcePaths` entry reports an unknown-type error when a cheatcode looks up a type by name (inline definitions still work).

Each test source is read and parsed once per run, at runner creation, serving both inline configuration and EIP-712 type collection. Consequently, every source listed in `testSourcePaths` must be parseable: a source compiled with a solc version older than 0.8 now rejects the whole run up front, even if it contains no inline configuration directives.
