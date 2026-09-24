//! Combined collection over the test sources.
//!
//! Each unique test source is read from disk and parsed with Slang exactly
//! once; both its inline test configuration (`forge-config:`/
//! `hardhat-config:` NatSpec directives) and its EIP-712 struct definitions
//! (served to the `eip712HashType`/`eip712HashStruct` cheatcodes) are
//! extracted from that same compilation unit. The unit is dropped afterwards
//! — nothing is cached beyond the extracted data.
//!
//! Every source named by `test_source_paths` is parsed; nothing is exempt and
//! nothing is skipped. Problems are accumulated across all of them — an
//! unreadable file, a solc version with no grammar, a source that does not
//! parse, ill-formed directives — and returned together, so one run reports
//! every problem rather than the first.

use std::{collections::HashMap, path::PathBuf, sync::Arc};

use edr_solidity_collector_eip712::collector::{
    collect_eip712_types_from_compilation_unit, Eip712TypeCollection,
};
use edr_solidity_parser_slang::{build_compilation_unit, ImportResolver, LanguageVersion};
use rayon::prelude::*;
use slang_solidity_v2::diagnostics::{DiagnosticExtensions as _, DiagnosticKind};

use crate::{
    inline_config::{collect_source_overrides_from_unit, line_of, SourceOverrides},
    test_source_error::{TestSourceCollectError, TestSourceErrorItem},
};

/// A source far enough out of sync with the grammar yields a syntax diagnostic
/// per token. The first few say what is wrong just as well as all of them, so
/// the rest are summarised as a count.
const MAX_REPORTED_PARSE_ERRORS: usize = 5;

/// A Solidity test source to collect from.
#[derive(Debug)]
pub(crate) struct TestSourceRoot {
    /// The identity this root is queried by — the compiled artifact's solc
    /// source name (e.g. the running test contract's `source`). Collections
    /// are keyed by this, because that is what a query has in hand.
    pub source: PathBuf,
    /// Absolute path to the file on disk, used to read and parse it.
    pub path: PathBuf,
    /// The grammar to parse the file with, mapped from the solc version it was
    /// compiled with. A source whose version maps to no grammar is reported as
    /// a problem instead of becoming a root, so parsing cannot fail on the
    /// version.
    pub version: LanguageVersion,
}

/// Everything extracted from one test source's single parse.
#[derive(Debug)]
pub(crate) struct SourceCollections {
    /// The EIP-712 struct definitions reachable from the source. Shared rather
    /// than copied because every suite declared in the source serves the same
    /// types.
    pub eip712_types: Arc<Eip712TypeCollection>,
    /// The successfully-parsed inline configuration, keyed by contract name.
    pub overrides: SourceOverrides,
}

/// Reads and parses every root, extracting both collections from each root's
/// single compilation unit, keyed by the root's source name.
///
/// Roots are parsed on rayon's global pool. Collection runs synchronously and
/// completes before any test suite is dispatched, so it never contends with
/// suite execution.
///
/// Every problem across every source is accumulated rather than
/// short-circuited, so one run reports them all.
pub(crate) fn collect_test_sources(
    roots: &[TestSourceRoot],
    import_resolver: &ImportResolver,
) -> Result<HashMap<PathBuf, SourceCollections>, Vec<TestSourceErrorItem>> {
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
/// Each root gets its own unit rather than sharing one. A merged unit would
/// widen `all_definitions()` across unrelated test files, so a struct name
/// that is unambiguous within one root could be rejected because another
/// root's import closure disagrees about it.
fn collect_root(
    root: &TestSourceRoot,
    import_resolver: &ImportResolver,
) -> Result<SourceCollections, Vec<TestSourceErrorItem>> {
    // Read the content up front because the NatSpec directives are recovered
    // from the raw source text. A build over a missing root only yields a
    // diagnostic and an empty unit, which must not be mistaken for "no types".
    let content = match std::fs::read_to_string(&root.path) {
        Ok(content) => content,
        Err(error) => {
            return Err(vec![TestSourceErrorItem {
                source_name: root.source.clone(),
                problem: TestSourceCollectError::RootFileNotFound {
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
    // doesn't fully parse could silently miss structs and directives. Report it
    // rather than collect half of it. Other diagnostic kinds — unresolvable
    // imports in particular, which are legitimately optional — keep degrading
    // gracefully.
    let mut syntax_diagnostics = unit
        .diagnostics()
        .iter()
        .filter(|diagnostic| {
            diagnostic.file_id() == file_id
                && matches!(diagnostic.kind(), DiagnosticKind::Syntax(_))
        })
        .peekable();

    if syntax_diagnostics.peek().is_some() {
        let mut reasons: Vec<String> = syntax_diagnostics
            .by_ref()
            .take(MAX_REPORTED_PARSE_ERRORS)
            .map(|diagnostic| {
                match line_of(&content, diagnostic.text_range().start) {
                    Ok(line) => format!("{} (line {line})", diagnostic.message()),
                    // The line is decoration on the reported problem
                    // because an offset we cannot place still reports its
                    // diagnostic.
                    Err(_unplaceable) => diagnostic.message(),
                }
            })
            .collect();

        let unreported = syntax_diagnostics.count();
        if unreported > 0 {
            reasons.push(format!("and {unreported} more"));
        }

        return Err(vec![TestSourceErrorItem {
            source_name: root.source.clone(),
            problem: TestSourceCollectError::SourceParseErrors { reasons }.into(),
        }]);
    }

    let overrides = collect_source_overrides_from_unit(&root.source, &content, &unit, &file_id)?;
    let eip712_types = Arc::new(collect_eip712_types_from_compilation_unit(&unit, &file_id));

    Ok(SourceCollections {
        eip712_types,
        overrides,
    })
}

#[cfg(test)]
mod tests {
    use std::{collections::HashSet, io::Write as _};

    use edr_solidity_parser_slang::language_version_for_solc;
    use semver::Version;

    use super::*;
    use crate::test_source_error::{InlineConfigDirectiveError, TestSourceProblem};

    fn temp_source(content: &str) -> tempfile::NamedTempFile {
        let mut file = tempfile::Builder::new()
            .suffix(".sol")
            .tempfile()
            .expect("temp file");
        file.write_all(content.as_bytes()).expect("write source");
        file
    }

    /// The solc source name most fixtures are collected under.
    const FIXTURE_SOURCE: &str = "project/C.t.sol";

    /// A solc version Slang has a grammar for. The fixtures are not about
    /// version mapping, so they all parse with the same one.
    fn fixture_grammar() -> LanguageVersion {
        language_version_for_solc(&Version::new(0, 8, 24)).expect("supported solc version")
    }

    fn root_for(file: &tempfile::NamedTempFile, source: &str) -> TestSourceRoot {
        TestSourceRoot {
            source: PathBuf::from(source),
            path: file.path().to_path_buf(),
            version: fixture_grammar(),
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
        let root = root_for(&file, FIXTURE_SOURCE);

        let collected = collect_root(&root, &ImportResolver::default())
            .unwrap_or_else(|errors| panic!("unexpected errors: {errors:?}"));

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
        let root = root_for(&file, FIXTURE_SOURCE);

        let collected = collect_root(&root, &ImportResolver::default())
            .unwrap_or_else(|errors| panic!("unexpected errors: {errors:?}"));

        assert!(collected.overrides.is_empty());
        assert!(collected.eip712_types.is_empty());
    }

    /// A partially-parsed source would silently miss structs and directives,
    /// so it is reported rather than half-collected.
    #[test]
    fn root_file_parse_errors_are_reported() {
        let file = temp_source(
            "// SPDX-License-Identifier: MIT
pragma solidity ^0.8.0;

struct Person { address wallet; string name; }

contract C {
    function testFoo() public { this is not solidity
}
",
        );
        let root = root_for(&file, FIXTURE_SOURCE);

        let errors = collect_test_sources(&[root], &ImportResolver::default())
            .expect_err("a source that does not parse cannot be collected");

        assert_eq!(errors.len(), 1, "{errors:#?}");
        assert_eq!(errors[0].source_name, PathBuf::from(FIXTURE_SOURCE));
        assert!(matches!(
            &errors[0].problem,
            TestSourceProblem::Source(TestSourceCollectError::SourceParseErrors { .. })
        ));
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
        let root = root_for(&file, FIXTURE_SOURCE);

        let collected = collect_root(&root, &ImportResolver::default())
            .unwrap_or_else(|errors| panic!("unexpected errors: {errors:?}"));

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
        let root = root_for(&file, "project/BadTest.t.sol");

        let errors = collect_root(&root, &ImportResolver::default()).expect_err("expected errors");

        assert_eq!(errors.len(), 1, "{errors:#?}");
        let TestSourceProblem::Directive(InlineConfigDirectiveError { function, line, .. }) =
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
            root_for(&first, "project/First.t.sol"),
            root_for(&second, "project/Second.t.sol"),
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
        let root = root_for(&file, "project/BadContractLevel.t.sol");

        let errors = collect_root(&root, &ImportResolver::default()).expect_err("expected errors");

        assert_eq!(errors.len(), 1, "{errors:#?}");
        let error = errors.first().expect("should contain an error");
        let TestSourceProblem::Directive(InlineConfigDirectiveError {
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
            version: fixture_grammar(),
        };

        let errors =
            collect_test_sources(&[root], &ImportResolver::default()).expect_err("expected errors");

        assert_eq!(errors.len(), 1);

        let error = errors.first().expect("should contain an error");
        assert!(matches!(
            &error.problem,
            TestSourceProblem::Source(TestSourceCollectError::RootFileNotFound { .. })
        ));
    }
}
