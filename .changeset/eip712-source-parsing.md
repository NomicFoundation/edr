---
"@nomicfoundation/edr": minor
---

BREAKING CHANGE: Removed the `eip712CanonicalTypes` field of `SolidityTestRunnerConfigArgs`. The `eip712HashType` and `eip712HashStruct` cheatcodes now resolve type names by parsing the running test contract's Solidity sources, instead of being handed a pre-computed list of canonical type strings.

To migrate: drop `eip712CanonicalTypes`; declare each struct you look up by name in the test contract's own source or a file it imports; give every test suite an entry in `testSourcePaths`; and add `importMappings` entries for the non-relative import paths those sources use.

Three consequences worth checking before you upgrade.

Name resolution is now scoped per suite rather than run-wide. A lookup sees only the structs declared in the running suite's own source and its transitive imports, where `eip712CanonicalTypes` was one list shared by every suite. A struct declared in an unrelated test file is no longer reachable, and an entry that was not backed by a real Solidity struct has no replacement at all.

`testSourcePaths` must now name the source of every test suite a run selects. Omitting the map (or passing an empty one) still disables collection entirely, but a non-empty map that misses a selected suite rejects that run before any test executes, rather than silently leaving the suite without inline configuration or EIP-712 types. Listing a source that cannot be parsed is safe: a source compiled with solc older than 0.8, or one Slang's grammar rejects, is skipped, and each suite it declares reports that on its `warnings` instead of failing the run.

`importMappings` keys are matched exactly, not by prefix, and an import with no entry stays unresolved. Parsing then degrades to what it can still reach, so a struct behind an unmapped import resolves as an unknown type rather than reporting the import as the cause.

Three new failure modes have no equivalent under the old configuration. A struct with a member EIP-712 cannot encode (a mapping, a function, a fixed-point number) is unusable, as is any struct referencing it. Two same-named structs in a source's import graph that declare different members leave the name ambiguous; the suite's own definition still wins a lookup by that name, but any struct *referencing* it is rejected, because a canonical type identifies its dependencies by bare name and could otherwise be encoded with the wrong body.

Each test source is read and Slang-parsed once per run, serving both inline configuration and EIP-712 collection. The cost is proportional to the number of test suites a run selects and the size of their import closures. Only selected suites are parsed, but note that this narrowing happens against the `TestFilter`, which the JS API does not expose — a consumer that already decides which suites to pass in sees no reduction from it.

Problems that do reject a run are reported through the structured `inlineConfigErrors` array on the thrown error, even when you use no inline configuration. `InlineConfigSourcePathNotProvided` is new. `InlineConfigInvalidSolcVersion` is removed from the `InlineConfigSourceProblem` union, since an unsupported solc version now skips the source instead of rejecting the run — narrowing on that `kind` no longer compiles.
