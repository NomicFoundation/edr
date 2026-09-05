//! Combined collection over the test sources.
//!
//! Each unique test source is read from disk and parsed with Slang exactly
//! once; both its inline test configuration (`forge-config:`/
//! `hardhat-config:` NatSpec directives) and its EIP-712 struct definitions
//! (served to the `eip712HashType`/`eip712HashStruct` cheatcodes) are
//! extracted from that same compilation unit. The unit is dropped afterwards
//! — nothing is cached beyond the extracted data.
//!
//! A source that does not parse is skipped rather than failing the run: it
//! may well use neither feature. The reason is reported as a warning on every
//! suite the source declares.

use std::{collections::HashMap, path::PathBuf, sync::Arc};

use edr_solidity_collector_eip712::collector::{
    collect_eip712_types_from_compilation_unit, Eip712TypeCollection,
};
use edr_solidity_parser_slang::{build_compilation_unit, ImportResolver, LanguageVersion};
use rayon::prelude::*;
use slang_solidity_v2::diagnostics::{DiagnosticExtensions as _, DiagnosticKind};

use crate::inline_config::{
    collect_source_overrides_from_unit,
    error::{InlineConfigCollectError, InlineConfigErrorItem},
    line_of, SourceOverrides,
};

/// A source far enough out of sync with the grammar yields a syntax diagnostic
/// per token; the first few say what is wrong just as well as all of them, and
/// the rest are summarised as a count.
const MAX_REPORTED_PARSE_ERRORS: usize = 5;

/// A Solidity test source to collect from.
#[derive(Clone, Debug)]
pub(crate) struct TestSourceRoot {
    /// The identity this root is queried by — the compiled artifact's solc
    /// source name (e.g. the running test contract's `source`). Collections
    /// are keyed by this, because that is what a query has in hand.
    pub source: PathBuf,
    /// Absolute path to the file on disk, used to read and parse it.
    pub path: PathBuf,
    /// The grammar to parse the file with, mapped from the solc version it was
    /// compiled with. A source whose version maps to no grammar never becomes
    /// a root, so parsing cannot fail on the version.
    pub version: LanguageVersion,
}

/// The outcome of collecting one test source.
#[derive(Clone, Debug)]
pub(crate) enum CollectedTestSource {
    /// The source was parsed; both collections come from its single unit.
    Collected(SourceCollections),
    /// The source could not be parsed, so nothing was collected from it.
    Skipped(UnparseableSource),
}

/// Everything extracted from one test source's single parse.
#[derive(Clone, Debug)]
pub(crate) struct SourceCollections {
    /// The EIP-712 struct definitions reachable from the source. Shared rather
    /// than copied: every suite declared in the source serves the same types.
    pub eip712_types: Arc<Eip712TypeCollection>,
    /// The successfully-parsed inline configuration, keyed by contract name.
    pub overrides: SourceOverrides,
}

/// A test source Slang's grammar rejects, so it yielded no inline
/// configuration and no EIP-712 types.
///
/// Slang is error-tolerant and yields a partial AST, which could silently miss
/// structs and directives, so nothing is collected from such a source. That is
/// not fatal on its own — a source using neither feature is unaffected — so
/// the run continues and every suite the source declares reports this as a
/// warning.
#[derive(Clone, Debug, thiserror::Error)]
#[error(
    "Skipped collecting inline configuration and EIP-712 types from \"{}\": the source did not \
     parse ({}). Inline configuration directives in this source have no effect, and the EIP-712 \
     cheatcodes cannot resolve type names declared in it.",
    .source_name.display(),
    .reasons.join("; ")
)]
pub(crate) struct UnparseableSource {
    /// The solc source name.
    source_name: PathBuf,
    /// The syntax diagnostics, each located at its source line.
    reasons: Vec<String>,
}

/// Reads and parses every root, extracting both collections from each root's
/// single compilation unit, keyed by the root's source name.
///
/// Roots are parsed on rayon's global pool. Collection runs synchronously and
/// completes before any test suite is dispatched, so it never contends with
/// suite execution.
///
/// Only a source that cannot be located or read, and ill-formed inline
/// configuration within one that can, are errors. Every such problem across
/// every source is accumulated rather than short-circuited, so one run reports
/// them all. A source Slang's grammar rejects is [`Skipped`] instead.
///
/// [`Skipped`]: CollectedTestSource::Skipped
pub(crate) fn collect_test_sources(
    roots: &[TestSourceRoot],
    import_resolver: &ImportResolver,
) -> Result<HashMap<PathBuf, CollectedTestSource>, Vec<InlineConfigErrorItem>> {
    let results: Vec<_> = roots
        .par_iter()
        .map(|root| (root.source.clone(), collect_root(root, import_resolver)))
        .collect();

    let (collected, errors) = results.into_iter().fold(
        (HashMap::new(), Vec::new()),
        |(mut collected, mut errors), (source, result)| {
            match result {
                Ok(collected_source) => {
                    collected.insert(source, collected_source);
                }
                Err(source_errors) => errors.extend(source_errors),
            }

            (collected, errors)
        },
    );

    if errors.is_empty() {
        Ok(collected)
    } else {
        Err(errors)
    }
}

/// Collects one root: read the file, build its compilation unit, and run both
/// extractions on it.
///
/// The two extractions run sequentially on purpose: Slang's CST nodes are
/// `Rc`-based, so the unit cannot be shared across threads; parallelism is
/// across roots instead.
fn collect_root(
    root: &TestSourceRoot,
    import_resolver: &ImportResolver,
) -> Result<CollectedTestSource, Vec<InlineConfigErrorItem>> {
    // Read the content up front: the NatSpec directives are recovered from the
    // raw source text, and a build over a missing root only yields a
    // diagnostic and an empty unit, which must not be mistaken for "no types".
    let content = match std::fs::read_to_string(&root.path) {
        Ok(content) => content,
        Err(error) => {
            return Err(vec![InlineConfigErrorItem {
                source_name: root.source.clone(),
                problem: InlineConfigCollectError::RootFileNotFound {
                    path: root.path.display().to_string(),
                    reason: error.to_string(),
                }
                .into(),
            }]);
        }
    };

    let unit = build_compilation_unit(&root.path, root.version, import_resolver);

    let file_id = root.path.to_string_lossy();

    // Slang is error-tolerant and yields a partial AST, so a root file that
    // doesn't fully parse could silently miss structs and directives; skip it
    // rather than collect half of it. Other diagnostic kinds — unresolvable
    // imports in particular, which are legitimately optional — keep degrading
    // gracefully.
    let parse_errors: Vec<String> = unit
        .diagnostics()
        .iter()
        .filter(|diagnostic| {
            diagnostic.file_id() == file_id
                && matches!(diagnostic.kind(), DiagnosticKind::Syntax(_))
        })
        .map(|diagnostic| {
            match line_of(&content, diagnostic.text_range().start) {
                Ok(line) => format!("{} (line {line})", diagnostic.message()),
                // The line is decoration on a warning; an offset we cannot
                // place is not worth failing the run over.
                Err(_unplaceable) => diagnostic.message(),
            }
        })
        .collect();

    if !parse_errors.is_empty() {
        let reported = parse_errors.len();
        let mut reasons: Vec<String> = parse_errors
            .into_iter()
            .take(MAX_REPORTED_PARSE_ERRORS)
            .collect();
        if reported > MAX_REPORTED_PARSE_ERRORS {
            reasons.push(format!("and {} more", reported - MAX_REPORTED_PARSE_ERRORS));
        }

        return Ok(CollectedTestSource::Skipped(UnparseableSource {
            source_name: root.source.clone(),
            reasons,
        }));
    }

    let overrides = collect_source_overrides_from_unit(&root.source, &content, &unit, &file_id)?;
    let eip712_types = Arc::new(collect_eip712_types_from_compilation_unit(&unit, &file_id));

    Ok(CollectedTestSource::Collected(SourceCollections {
        eip712_types,
        overrides,
    }))
}

#[cfg(test)]
mod tests {
    use std::{collections::HashSet, io::Write as _};

    use edr_solidity_parser_slang::language_version_for_solc;
    use semver::Version;

    use super::*;
    use crate::inline_config::error::{InlineConfigDirectiveError, InlineConfigProblem};

    /// Unwraps the collections of a source expected to have been parsed.
    fn collections(source: CollectedTestSource) -> SourceCollections {
        match source {
            CollectedTestSource::Collected(collections) => collections,
            CollectedTestSource::Skipped(reason) => panic!("unexpectedly skipped: {reason}"),
        }
    }

    fn temp_source(content: &str) -> tempfile::NamedTempFile {
        let mut file = tempfile::Builder::new()
            .suffix(".sol")
            .tempfile()
            .expect("temp file");
        file.write_all(content.as_bytes()).expect("write source");
        file
    }

    fn root_for(file: &tempfile::NamedTempFile, source: &str, version: Version) -> TestSourceRoot {
        TestSourceRoot {
            source: PathBuf::from(source),
            path: file.path().to_path_buf(),
            version: language_version_for_solc(&version).expect("supported solc version"),
        }
    }

    #[test]
    fn collects_both_from_a_single_parse() {
        let file = temp_source(
            "// SPDX-License-Identifier: MIT
pragma solidity ^0.8.0;

struct Person { address wallet; string name; }

contract C {
    /// forge-config: default.fuzz.runs = 5
    function testFoo(uint256 x) public {}
}
",
        );
        let root = root_for(&file, "project/C.t.sol", Version::new(0, 8, 24));

        let collected = collections(
            collect_root(&root, &ImportResolver::default())
                .unwrap_or_else(|errors| panic!("unexpected errors: {errors:?}")),
        );

        let overrides = collected.overrides.get("C").expect("C has overrides");
        assert_eq!(overrides.functions.len(), 1);
        assert_eq!(overrides.functions[0].function_name, "testFoo");

        assert_eq!(
            collected
                .eip712_types
                .get("Person")
                .expect("Person is collected")
                .canonical_definition(),
            "Person(address wallet,string name)"
        );
    }

    #[test]
    fn source_without_directives_or_structs_yields_empty_collections() {
        let file = temp_source(
            "// SPDX-License-Identifier: MIT
pragma solidity ^0.8.0;

contract C {
    function testFoo() public {}
}
",
        );
        let root = root_for(&file, "project/C.t.sol", Version::new(0, 8, 24));

        let collected = collections(
            collect_root(&root, &ImportResolver::default())
                .unwrap_or_else(|errors| panic!("unexpected errors: {errors:?}")),
        );

        assert!(collected.overrides.is_empty());
        assert!(collected.eip712_types.is_empty());
    }

    /// A partially-parsed source would silently miss structs and directives,
    /// so nothing is collected from it — but the run continues.
    #[test]
    fn root_file_parse_errors_are_skipped() {
        let file = temp_source(
            "// SPDX-License-Identifier: MIT
pragma solidity ^0.8.0;

struct Person { address wallet; string name; }

contract C {
    function testFoo() public { this is not solidity
}
",
        );
        let root = root_for(&file, "project/C.t.sol", Version::new(0, 8, 24));

        let collected = collect_test_sources(&[root], &ImportResolver::default())
            .unwrap_or_else(|errors| panic!("unexpected errors: {errors:?}"));

        let source = &collected[&PathBuf::from("project/C.t.sol")];
        let CollectedTestSource::Skipped(reason) = source else {
            panic!("a source that does not parse cannot be collected");
        };
        // The warning names the source and says what stops working.
        assert!(reason.to_string().contains("project/C.t.sol"), "{reason}");
        assert!(reason.to_string().contains("EIP-712"), "{reason}");
    }

    /// Both directive prefixes are recognized, and a directive that is not in
    /// a NatSpec comment is not a directive at all.
    #[test]
    fn hardhat_prefix_is_collected_and_plain_comments_are_not() {
        let file = temp_source(
            "// SPDX-License-Identifier: MIT
pragma solidity ^0.8.0;

contract C {
    /// hardhat-config: default.fuzz.runs = 11
    function testHardhatPrefix(uint256 x) public {}

    // not natspec: forge-config: default.fuzz.runs = 999
    function testPlainComment() public {}
}
",
        );
        let root = root_for(&file, "project/C.t.sol", Version::new(0, 8, 24));

        let collected = collections(
            collect_root(&root, &ImportResolver::default())
                .unwrap_or_else(|errors| panic!("unexpected errors: {errors:?}")),
        );

        let overrides = collected.overrides.get("C").expect("C has overrides");
        assert_eq!(overrides.functions.len(), 1, "{:#?}", overrides.functions);
        assert_eq!(overrides.functions[0].function_name, "testHardhatPrefix");
        assert_eq!(
            overrides.functions[0].config.fuzz.as_ref().unwrap().runs,
            Some(11)
        );
    }

    /// Exactly one problem per malformed function — not one per bad directive
    /// — reported at the first offending line, and a well-formed sibling in
    /// the same contract is unaffected.
    #[test]
    fn one_problem_per_malformed_function_at_its_first_bad_directive() {
        let file = temp_source(
            "// SPDX-License-Identifier: MIT
pragma solidity ^0.8.0;

contract BadTest {
    /// forge-config: default.fuzz.runs = -1
    /// forge-config: fuzz.maxTestRejects = -2
    function testFuzz(uint256 x) public {}

    /// forge-config: default.fuzz.runs = 5
    function testValid(uint256 x) public {}
}
",
        );
        let root = root_for(&file, "project/BadTest.t.sol", Version::new(0, 8, 24));

        let errors = collect_root(&root, &ImportResolver::default()).expect_err("expected errors");

        assert_eq!(errors.len(), 1, "{errors:#?}");
        let InlineConfigProblem::Directive(InlineConfigDirectiveError { function, line, .. }) =
            &errors[0].problem
        else {
            panic!("expected a directive problem, got {:#?}", errors[0].problem);
        };
        assert_eq!(function.as_deref(), Some("testFuzz"));
        // The `runs = -1` line, not the `-2` one below it.
        assert_eq!(*line, 5);
    }

    /// Several sources failing at once are reported together, so one run
    /// surfaces every problem rather than the first.
    #[test]
    fn problems_across_sources_are_reported_together() {
        let bad = "// SPDX-License-Identifier: MIT
pragma solidity ^0.8.0;

contract Bad {
    /// forge-config: default.fuzz.runs = -1
    function testFuzz(uint256 x) public {}
}
";
        let first = temp_source(bad);
        let second = temp_source(bad);
        let roots = [
            root_for(&first, "project/First.t.sol", Version::new(0, 8, 24)),
            root_for(&second, "project/Second.t.sol", Version::new(0, 8, 24)),
        ];

        let errors =
            collect_test_sources(&roots, &ImportResolver::default()).expect_err("expected errors");

        assert_eq!(errors.len(), 2, "{errors:#?}");
        let sources: HashSet<_> = errors.iter().map(|item| item.source_name.clone()).collect();
        assert_eq!(
            sources,
            HashSet::from([
                PathBuf::from("project/First.t.sol"),
                PathBuf::from("project/Second.t.sol"),
            ])
        );
    }

    /// A contract-level directive carries no function, so its problem is
    /// reported against the contract alone.
    #[test]
    fn malformed_contract_level_directive_is_reported_without_a_function() {
        let file = temp_source(
            "// SPDX-License-Identifier: MIT
pragma solidity ^0.8.0;

/// forge-config: default.fuzz.runs = -1
contract BadContractLevel {
    /// forge-config: default.fuzz.runs = 5
    function testValid(uint256 x) public {}
}
",
        );
        let root = root_for(
            &file,
            "project/BadContractLevel.t.sol",
            Version::new(0, 8, 24),
        );

        let errors = collect_root(&root, &ImportResolver::default()).expect_err("expected errors");

        assert_eq!(errors.len(), 1, "{errors:#?}");
        let error = errors.first().expect("should contain an error");
        let InlineConfigProblem::Directive(InlineConfigDirectiveError {
            contract,
            function,
            line,
            ..
        }) = &error.problem
        else {
            panic!("expected a directive problem, got {:#?}", error.problem);
        };
        assert_eq!(contract, "BadContractLevel");
        assert_eq!(*function, None);
        assert_eq!(*line, 4);

        // The rendered report names the contract without a function.
        assert!(error.to_string().contains("BadContractLevel:"), "{error}");
    }

    #[test]
    fn missing_root_file_is_a_source_error() {
        let root = TestSourceRoot {
            source: PathBuf::from("project/Missing.t.sol"),
            path: PathBuf::from("/does/not/exist/Missing.t.sol"),
            version: LanguageVersion::V0_8_24,
        };

        let errors =
            collect_test_sources(&[root], &ImportResolver::default()).expect_err("expected errors");

        assert_eq!(errors.len(), 1);

        let error = errors.first().expect("should contain an error");
        assert!(matches!(
            &error.problem,
            InlineConfigProblem::Source(InlineConfigCollectError::RootFileNotFound { .. })
        ));
    }
}
