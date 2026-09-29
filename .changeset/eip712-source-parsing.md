---
"@nomicfoundation/edr": minor
---

BREAKING CHANGE: Removed the `eip712CanonicalTypes` field of `SolidityTestRunnerConfigArgs`. The `eip712HashType` and `eip712HashStruct` cheatcodes now resolve type names by parsing the running test contract's Solidity sources. They are no longer handed a pre-computed list of canonical type strings.

To migrate:

- drop `eip712CanonicalTypes`;
- declare each struct you look up by name in the test contract's own source, or in a file it imports;
- give every test suite an entry in `testSourcePaths`;
- add `importMappings` entries for the non-relative import paths those sources use.

The consequences below are worth checking before you upgrade.

Name resolution is now scoped per suite rather than run-wide. A lookup sees only the structs declared in the running suite's own source and its transitive imports. `eip712CanonicalTypes` was instead one list shared by every suite. A struct declared in an unrelated test file is no longer reachable. An entry never backed by a real Solidity struct has no replacement at all.

`testSourcePaths` must now name the source of every test suite a run selects, with no exceptions. Every source backing a selected suite is parsed, and an entry no selected suite uses is never opened. A missing entry or an unreadable file rejects the run before any test executes. So does a source that does not parse, one whose import does not parse, or one compiled with a solc version older than 0.8. Nothing is left silently uncollected. Every such problem across every source is accumulated and reported together, so one run surfaces them all.

Both features require solc 0.8 or newer, and there is no per-source exemption. Omitting the map, or passing an empty one, still disables collection entirely. That is now the only way to run test sources solc 0.8 cannot parse. The whole run then loses inline configuration and EIP-712 types.

`importMappings` keys are matched exactly, not by prefix, and an import with no entry stays unresolved. Parsing then degrades to what it can still reach. A struct behind an unmapped import therefore resolves as an unknown type, rather than reporting the import as the cause.

Three new failure modes have no equivalent under the old configuration. EIP-712 cannot encode a mapping, a function, or a fixed-point number. A struct with such a member is unusable, as is any struct referencing it. Two same-named structs in a source's import graph may declare different members, which leaves the name ambiguous. The suite's own definition still wins a lookup by that name, but any struct _referencing_ it is rejected. A canonical type identifies its dependencies by bare name, so the wrong body could otherwise be encoded.

Each test source is read and Slang-parsed once per run, serving both inline configuration and EIP-712 collection. The cost is proportional to the number of test suites a run selects and the size of their import closures. Only selected suites are parsed, but that narrowing happens against the `TestFilter`, which the JS API does not expose. A consumer that already decides which suites to pass in sees no reduction from it.

BREAKING CHANGE: the structured error array on the thrown error is renamed from `inlineConfigErrors` to `testSourceErrors`. The source-level types it carries are renamed to match, because they now report problems that have nothing to do with inline configuration. A run using no directives at all can be rejected by one of them:

- `InlineConfigError` changed to `TestSourceError`;
- `InlineConfigSourceError` changed to `TestSourceFileError`;
- `InlineConfigSourceProblem` changed to `TestSourceFileProblem`;
- `InlineConfigSourceFileNotFound` changed to `TestSourceFileNotFound`;
- `InlineConfigSourcePathNotProvided` changed to `TestSourcePathNotProvided`;
- `InlineConfigDirectiveLocation` changed to `TestSourceDirectiveLocation`.

The directive-level types keep their `InlineConfig` names, because they do describe inline configuration.

Problems that reject a run are reported through the structured `testSourceErrors` array on the thrown error, even when you use no inline configuration. The thrown error's `message` now begins `Could not collect from the test sources:` rather than `Found invalid inline configuration in test sources:`. The new wording covers problems that are not about directives. `TestSourcePathNotProvided` and `TestSourceParseErrors` are new, and `InlineConfigInvalidSolcVersion` is replaced by `TestSourceUnsupportedSolcVersion`, which carries the offending `version`. Narrowing on the old `kind` no longer compiles.
