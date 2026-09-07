---
"@nomicfoundation/edr": minor
---

BREAKING CHANGE: Removed the `eip712CanonicalTypes` field of `SolidityTestRunnerConfigArgs`. The `eip712HashType` and `eip712HashStruct` cheatcodes now resolve type names by parsing the running test contract's Solidity sources, instead of being handed a pre-computed list of canonical type strings.

To migrate:

- drop `eip712CanonicalTypes`;
- declare each struct you look up by name in the test contract's own source, or in a file it imports;
- give every test suite an entry in `testSourcePaths`;
- add `importMappings` entries for the non-relative import paths those sources use.

Consequences worth checking before you upgrade.

Name resolution is now scoped per suite rather than run-wide. A lookup sees only the structs declared in the running suite's own source and its transitive imports, where `eip712CanonicalTypes` was one list shared by every suite. A struct declared in an unrelated test file is no longer reachable, and an entry that was not backed by a real Solidity struct has no replacement at all.

`testSourcePaths` must now name the source of every test suite a run selects, with no exceptions, and every source backing a selected suite is parsed. An entry no selected suite uses is never opened. A missing entry, an unreadable file, a source that does not parse, or a solc version older than 0.8 rejects the run before any test executes. Nothing is left silently uncollected. Every such problem across every source is accumulated and reported together, so one run surfaces them all.

Both features require solc 0.8 or newer, and there is no per-source exemption. Omitting the map (or passing an empty one) still disables collection entirely, and that is now the only way to run test sources solc 0.8 cannot parse — at the cost of losing inline configuration and EIP-712 types for the whole run.

`importMappings` keys are matched exactly, not by prefix, and an import with no entry stays unresolved. Parsing then degrades to what it can still reach, so a struct behind an unmapped import resolves as an unknown type rather than reporting the import as the cause.

Three new failure modes have no equivalent under the old configuration. A struct with a member EIP-712 cannot encode (a mapping, a function, a fixed-point number) is unusable, as is any struct referencing it. Two same-named structs in a source's import graph that declare different members leave the name ambiguous; the suite's own definition still wins a lookup by that name, but any struct *referencing* it is rejected, because a canonical type identifies its dependencies by bare name and could otherwise be encoded with the wrong body.

Each test source is read and Slang-parsed once per run, serving both inline configuration and EIP-712 collection. The cost is proportional to the number of test suites a run selects and the size of their import closures. Only selected suites are parsed, but that narrowing happens against the `TestFilter`, which the JS API does not expose. A consumer that already decides which suites to pass in sees no reduction from it.

BREAKING CHANGE: the structured error array on the thrown error is renamed from `inlineConfigErrors` to `testSourceErrors`, and the source-level types it carries are renamed to match. They now report problems that have nothing to do with inline configuration, so a run using no directives at all can be rejected by one: `InlineConfigError` becomes `TestSourceError`, `InlineConfigSourceError` becomes `TestSourceFileError`, `InlineConfigSourceProblem` becomes `TestSourceFileProblem`, and the members `InlineConfigSourceFileNotFound`, `InlineConfigSourcePathNotProvided` and `InlineConfigDirectiveLocation` become `TestSourceFileNotFound`, `TestSourcePathNotProvided` and `TestSourceDirectiveLocation`. The directive-level types keep their `InlineConfig` names, because they do describe inline configuration.

Problems that do reject a run are reported through the structured `testSourceErrors` array on the thrown error, even when you use no inline configuration. The thrown error's `message` now begins `Could not collect from the test sources:` rather than `Found invalid inline configuration in test sources:`, since it covers problems that are not about directives. `TestSourcePathNotProvided` is new. `InlineConfigInvalidSolcVersion` is replaced by `TestSourceUnsupportedSolcVersion`, which carries the offending `version`, and `TestSourceParseErrors` is new — narrowing on the old `kind` no longer compiles.
