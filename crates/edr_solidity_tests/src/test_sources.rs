//! Combined collection over the test sources.
//!
//! The selected test sources are parsed with Slang in one compilation unit per
//! language version, each source being a root of its unit. A file imported by
//! several roots is therefore parsed and analysed once instead of once per
//! root. Each root's inline test configuration (`forge-config:`/
//! `hardhat-config:` NatSpec directives) and EIP-712 struct definitions are
//! then extracted from that shared unit. The EIP-712 collection is scoped to
//! the files the root reaches, while directives come from the root's own text.
//! The units are dropped afterwards, so nothing is cached beyond the extracted
//! data.
//!
//! Only the sources of the suites a run selects are parsed.
//! `MultiContractRunner::test` applies the test filter before collection runs,
//! and `test_source_roots` builds roots only from the selected artifacts, so an
//! entry in `test_source_paths` that no selected suite uses is never opened.
//! Problems are accumulated across every source that is read, such as an
//! unreadable file, a solc version with no grammar, a source that does not
//! parse, or an ill-formed directive, and returned together, so one run reports
//! every problem rather than the first.

use std::{
    borrow::Cow,
    collections::{BTreeMap, HashMap},
    path::PathBuf,
    sync::Arc,
};

use edr_solidity_collector_eip712::collector::{Eip712TypeCollection, UnitStructs};
use edr_solidity_parser_slang::{
    compilation::{build_multi_root_compilation_unit, file_id_of, MultiRootCompilation},
    resolver::{ImportResolver, ProvidedSources},
    LanguageVersion,
};
use rayon::prelude::*;
use slang_solidity_v2::{
    compilation::CompilationUnit,
    diagnostics::{Diagnostic, DiagnosticExtensions as _, DiagnosticKind},
};

use crate::{
    inline_config::{
        collect_source_overrides_from_unit, line_of, InlineConfigProfiles, SourceOverrides,
    },
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

/// Everything extracted from one test source.
#[derive(Debug)]
pub(crate) struct SourceCollections {
    /// The EIP-712 struct definitions reachable from the source. Shared rather
    /// than copied because every suite declared in the source serves the same
    /// types.
    pub eip712_types: Arc<Eip712TypeCollection>,
    /// The successfully-parsed inline configuration, keyed by contract name.
    pub overrides: SourceOverrides,
}

/// Reads and parses every root, extracting both collections for each root,
/// keyed by the root's source name.
///
/// Roots compiled with the same solc grammar share one compilation unit.
/// Extraction from a built unit runs on rayon's global pool. Collection runs
/// synchronously and completes before any test suite is dispatched, so it never
/// contends with suite execution.
///
/// Every problem across every source is accumulated rather than
/// short-circuited, so one run reports them all.
pub(crate) fn collect_test_sources(
    roots: &[TestSourceRoot],
    import_resolver: &ImportResolver,
    profiles: &InlineConfigProfiles,
) -> Result<HashMap<PathBuf, SourceCollections>, Vec<TestSourceErrorItem>> {
    // Roots of different versions cannot share a unit because a unit parses
    // every file it holds with one grammar.
    let mut roots_by_version: BTreeMap<LanguageVersion, Vec<&TestSourceRoot>> = BTreeMap::new();
    for root in roots {
        roots_by_version.entry(root.version).or_default().push(root);
    }

    let mut collected = HashMap::new();
    let mut errors = Vec::new();
    for (version, version_roots) in roots_by_version {
        // Build the next version's unit only after this one is dropped.
        let MultiRootCompilation { unit, sources } = build_multi_root_compilation_unit(
            version_roots.iter().map(|root| root.path.as_path()),
            version,
            import_resolver,
        );

        // Several source names can point at one file.
        let mut roots_by_file_id: HashMap<String, Vec<&TestSourceRoot>> = HashMap::new();
        for root in &version_roots {
            roots_by_file_id
                .entry(file_id_of(&root.path))
                .or_default()
                .push(root);
        }
        let roots_for = |file_id: &str| -> &[&TestSourceRoot] {
            roots_by_file_id
                .get(file_id)
                .expect("every root of this version is keyed by its file id")
        };

        for (file_id, error) in sources.unreadable_roots() {
            for root in roots_for(file_id) {
                errors.push(TestSourceErrorItem {
                    source_name: root.source.clone(),
                    problem: TestSourceCollectError::RootFileNotFound {
                        path: root.path.display().to_string(),
                        reason: error.to_string(),
                    }
                    .into(),
                });
            }
        }

        // Slang is error-tolerant and yields a partial AST, so a file that
        // doesn't fully parse could silently miss structs and directives.
        let syntax_diagnostics: Vec<&Diagnostic> = unit
            .diagnostics()
            .iter()
            .filter(|diagnostic| matches!(diagnostic.kind(), DiagnosticKind::Syntax(_)))
            .collect();

        let structs = UnitStructs::from_unit(&unit);
        let diagnostic_texts = read_back_diagnostic_texts(&syntax_diagnostics, &sources);
        let version_unit = &VersionUnit {
            unit: &unit,
            sources: &sources,
            structs: &structs,
            syntax_diagnostics: &syntax_diagnostics,
            diagnostic_texts: &diagnostic_texts,
        };

        let results: Vec<_> = sources
            .root_contents()
            .par_iter()
            .flat_map_iter(|(file_id, root_content)| {
                roots_for(file_id).iter().map(move |root| {
                    let collections =
                        collect_root(root, file_id, root_content, version_unit, profiles);

                    (root.source.clone(), collections)
                })
            })
            .collect();

        for (source, result) in results {
            match result {
                Ok(collected_source) => {
                    collected.insert(source, collected_source);
                }
                Err(source_errors) => errors.extend(source_errors),
            }
        }
    }

    if errors.is_empty() {
        Ok(collected)
    } else {
        // Roots are extracted in map order, so sort the report by source to
        // keep it identical on every run. The sort is stable, so a source's
        // own problems keep their order.
        errors.sort_by(|left, right| left.source_name.cmp(&right.source_name));
        Err(errors)
    }
}

/// The shared unit of one language version and what every root extracts
/// from it.
struct VersionUnit<'a> {
    unit: &'a CompilationUnit,
    sources: &'a ProvidedSources,
    structs: &'a UnitStructs,
    syntax_diagnostics: &'a [&'a Diagnostic],
    /// The text of every file a syntax diagnostic points into, by file id, or
    /// `None` when it could not be read back.
    diagnostic_texts: &'a HashMap<&'a str, Option<Cow<'a, str>>>,
}

/// Extracts one root's collections from the unit its version was built into.
///
/// EIP-712 collection is scoped to the files the root reaches through resolved
/// imports. A struct declared in another root's closure neither shadows the
/// root's own nor is served to it. Inline-config directives come only from the
/// root's own text. A syntax error anywhere in the closure rejects the root,
/// because the EIP-712 types are collected from the whole closure. An
/// unresolvable import does not reject the root, because such imports are
/// legitimately optional.
fn collect_root(
    root: &TestSourceRoot,
    file_id: &str,
    root_content: &str,
    version_unit: &VersionUnit<'_>,
    profiles: &InlineConfigProfiles,
) -> Result<SourceCollections, Vec<TestSourceErrorItem>> {
    let reachable = version_unit.sources.reachable_from(file_id);

    let mut reachable_diagnostics = version_unit
        .syntax_diagnostics
        .iter()
        .filter(|diagnostic| reachable.contains(diagnostic.file_id()))
        .peekable();

    if reachable_diagnostics.peek().is_some() {
        let mut reasons: Vec<String> = reachable_diagnostics
            .by_ref()
            .take(MAX_REPORTED_PARSE_ERRORS)
            .map(|diagnostic| {
                describe_syntax_diagnostic(diagnostic, file_id, version_unit.diagnostic_texts)
            })
            .collect();

        let unreported = reachable_diagnostics.count();
        if unreported > 0 {
            reasons.push(format!("and {unreported} more"));
        }

        return Err(vec![TestSourceErrorItem {
            source_name: root.source.clone(),
            problem: TestSourceCollectError::SourceParseErrors { reasons }.into(),
        }]);
    }

    let overrides = collect_source_overrides_from_unit(
        &root.source,
        root_content,
        version_unit.unit,
        file_id,
        profiles,
    )?;
    let eip712_types = Arc::new(version_unit.structs.types_for(file_id, &reachable));

    Ok(SourceCollections {
        eip712_types,
        overrides,
    })
}

/// Reads back the text of every file a syntax diagnostic points into, once per
/// unit. A root's text is already retained, and an import's is read from disk
/// because the unit does not expose it.
fn read_back_diagnostic_texts<'a>(
    syntax_diagnostics: &[&'a Diagnostic],
    sources: &'a ProvidedSources,
) -> HashMap<&'a str, Option<Cow<'a, str>>> {
    let mut texts = HashMap::new();
    for diagnostic in syntax_diagnostics {
        let file_id = diagnostic.file_id();
        if texts.contains_key(file_id) {
            continue;
        }

        let text = match sources.root_contents().get(file_id) {
            Some(content) => Some(Cow::Borrowed(content.as_str())),
            None => std::fs::read_to_string(file_id)
                .inspect_err(|error| {
                    // Slang read this file moments ago to parse it, so failing
                    // to read it back means it changed underneath the run.
                    tracing::error!(
                        file = file_id,
                        %error,
                        "could not read back an import to locate its syntax error"
                    );
                })
                .ok()
                .map(Cow::Owned),
        };
        texts.insert(file_id, text);
    }

    texts
}

/// Formats a syntax diagnostic as its message followed by where it was found:
/// the line, and the file when it is an import rather than the root.
///
/// The location is decoration on the reported problem because an offset we
/// cannot place, or an import we cannot read back, still reports its
/// diagnostic.
fn describe_syntax_diagnostic(
    diagnostic: &Diagnostic,
    root_file_id: &str,
    diagnostic_texts: &HashMap<&str, Option<Cow<'_, str>>>,
) -> String {
    let diagnostic_file_id = diagnostic.file_id();
    let is_root = diagnostic_file_id == root_file_id;

    let mut location = Vec::new();
    if let Some(line) = diagnostic_texts
        .get(diagnostic_file_id)
        .and_then(|content| content.as_deref())
        .and_then(|content| line_of(content, diagnostic.text_range().start).ok())
    {
        location.push(format!("line {line}"));
    }
    if !is_root {
        location.push(format!("in {diagnostic_file_id}"));
    }

    if location.is_empty() {
        diagnostic.message()
    } else {
        format!("{} ({})", diagnostic.message(), location.join(" "))
    }
}

#[cfg(test)]
mod tests {
    use std::{collections::HashSet, io::Write as _, path::Path};

    use edr_solidity_parser_slang::compilation::language_version_for_solc;
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

    /// A second supported grammar, so a root using it lands in a different
    /// unit than one using [`fixture_grammar`].
    fn older_fixture_grammar() -> LanguageVersion {
        language_version_for_solc(&Version::new(0, 8, 4)).expect("supported solc version")
    }

    fn root_for(file: &tempfile::NamedTempFile, source: &str) -> TestSourceRoot {
        TestSourceRoot {
            source: PathBuf::from(source),
            path: file.path().to_path_buf(),
            version: fixture_grammar(),
        }
    }

    /// Collects `roots` with no import mappings and the default profile.
    fn collect_with_defaults(
        roots: &[TestSourceRoot],
    ) -> Result<HashMap<PathBuf, SourceCollections>, Vec<TestSourceErrorItem>> {
        collect_test_sources(
            roots,
            &ImportResolver::default(),
            &InlineConfigProfiles::default(),
        )
    }

    fn unwrap_collected<T>(result: Result<T, Vec<TestSourceErrorItem>>) -> T {
        result.unwrap_or_else(|errors| panic!("unexpected errors: {errors:?}"))
    }

    /// Collects a lone root and returns its own collections.
    fn collect_one(root: TestSourceRoot) -> Result<SourceCollections, Vec<TestSourceErrorItem>> {
        let source = root.source.clone();

        collect_with_defaults(&[root])
            .map(|mut collected| collected.remove(&source).expect("the root was collected"))
    }

    fn root_at(path: PathBuf, source: &str) -> TestSourceRoot {
        TestSourceRoot {
            source: PathBuf::from(source),
            path,
            version: fixture_grammar(),
        }
    }

    fn write_source(dir: &tempfile::TempDir, name: &str, content: &str) -> PathBuf {
        let path = dir.path().join(name);
        std::fs::write(&path, content).expect("write source");
        path
    }

    fn collect_all(roots: &[TestSourceRoot]) -> HashMap<PathBuf, SourceCollections> {
        unwrap_collected(collect_with_defaults(roots))
    }

    fn collected_for<'a>(
        collected: &'a HashMap<PathBuf, SourceCollections>,
        source: &str,
    ) -> &'a SourceCollections {
        collected
            .get(Path::new(source))
            .unwrap_or_else(|| panic!("`{source}` was not collected"))
    }

    fn canonical_type(collected: &SourceCollections, name: &str) -> String {
        collected
            .eip712_types
            .get(name)
            .unwrap_or_else(|error| panic!("expected `{name}`: {error}"))
            .canonical_definition()
            .to_owned()
    }

    #[test]
    fn collects_overrides_and_eip712_types_from_one_source() {
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

        let collected = unwrap_collected(collect_one(root));

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

        let collected = unwrap_collected(collect_one(root));

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

        let errors = collect_test_sources(
            &[root],
            &ImportResolver::default(),
            &InlineConfigProfiles::default(),
        )
        .expect_err("a source that does not parse cannot be collected");

        assert_eq!(errors.len(), 1, "{errors:#?}");
        assert_eq!(errors[0].source_name, PathBuf::from(FIXTURE_SOURCE));
        assert!(matches!(
            &errors[0].problem,
            TestSourceProblem::Source(TestSourceCollectError::SourceParseErrors { .. })
        ));
    }

    #[test]
    fn imported_file_parse_errors_are_reported_with_their_file() {
        let dir = tempfile::tempdir().expect("temp dir");
        std::fs::write(
            dir.path().join("Dep.sol"),
            "pragma solidity ^0.8.0;

struct Person { address wallet; string name;
",
        )
        .expect("write dependency");
        let root_path = dir.path().join("C.t.sol");
        std::fs::write(
            &root_path,
            "pragma solidity ^0.8.0;

import {Person} from \"./Dep.sol\";

contract C {
    function testFoo() public {}
}
",
        )
        .expect("write root");
        let root = TestSourceRoot {
            source: PathBuf::from(FIXTURE_SOURCE),
            path: root_path,
            version: fixture_grammar(),
        };

        let errors = collect_test_sources(
            &[root],
            &ImportResolver::default(),
            &InlineConfigProfiles::default(),
        )
        .expect_err("an import that does not parse cannot be collected");

        assert_eq!(errors.len(), 1, "{errors:#?}");
        assert_eq!(errors[0].source_name, PathBuf::from(FIXTURE_SOURCE));
        let TestSourceProblem::Source(TestSourceCollectError::SourceParseErrors { reasons }) =
            &errors[0].problem
        else {
            panic!("expected parse errors, got {:?}", errors[0].problem);
        };
        assert!(
            !reasons.is_empty() && reasons.iter().all(|reason| reason.contains("Dep.sol")),
            "{reasons:?}"
        );
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

        let collected = unwrap_collected(collect_one(root));

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

        let errors = collect_one(root).expect_err("expected errors");

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

        let errors = collect_test_sources(
            &roots,
            &ImportResolver::default(),
            &InlineConfigProfiles::default(),
        )
        .expect_err("expected errors");

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

        let errors = collect_one(root).expect_err("expected errors");

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

        let errors = collect_test_sources(
            &[root],
            &ImportResolver::default(),
            &InlineConfigProfiles::default(),
        )
        .expect_err("expected errors");

        assert_eq!(errors.len(), 1);

        let error = errors.first().expect("should contain an error");
        assert!(matches!(
            &error.problem,
            TestSourceProblem::Source(TestSourceCollectError::RootFileNotFound { .. })
        ));
    }

    #[test]
    fn roots_sharing_a_unit_resolve_names_in_their_own_closure() {
        let dir = tempfile::tempdir().expect("temp dir");
        write_source(
            &dir,
            "WalletDep.sol",
            "pragma solidity ^0.8.0;\n\nstruct Person { address wallet; }\n",
        );
        write_source(
            &dir,
            "NameDep.sol",
            "pragma solidity ^0.8.0;\n\nstruct Person { string name; }\n",
        );
        let wallet_root = write_source(
            &dir,
            "Wallet.t.sol",
            "pragma solidity ^0.8.0;\n\nimport {Person} from \"./WalletDep.sol\";\n\ncontract WalletTest {}\n",
        );
        let name_root = write_source(
            &dir,
            "Name.t.sol",
            "pragma solidity ^0.8.0;\n\nimport {Person} from \"./NameDep.sol\";\n\ncontract NameTest {}\n",
        );
        let wallet_source = "project/Wallet.t.sol";
        let name_source = "project/Name.t.sol";

        let collected = collect_all(&[
            root_at(wallet_root, wallet_source),
            root_at(name_root, name_source),
        ]);

        assert_eq!(
            canonical_type(collected_for(&collected, wallet_source), "Person"),
            "Person(address wallet)"
        );
        assert_eq!(
            canonical_type(collected_for(&collected, name_source), "Person"),
            "Person(string name)"
        );
    }

    #[test]
    fn a_root_does_not_see_another_roots_imports() {
        let dir = tempfile::tempdir().expect("temp dir");
        write_source(
            &dir,
            "Dep.sol",
            "pragma solidity ^0.8.0;\n\nstruct Person { address wallet; }\n",
        );
        let importer = write_source(
            &dir,
            "Importer.t.sol",
            "pragma solidity ^0.8.0;\n\nimport {Person} from \"./Dep.sol\";\n\ncontract ImporterTest {}\n",
        );
        let other = write_source(
            &dir,
            "Other.t.sol",
            "pragma solidity ^0.8.0;\n\ncontract OtherTest {}\n",
        );

        let other_source = "project/Other.t.sol";

        let collected = collect_all(&[
            root_at(importer, "project/Importer.t.sol"),
            root_at(other, other_source),
        ]);

        let other = collected_for(&collected, other_source);
        assert!(other.eip712_types.is_empty());
        assert!(other.eip712_types.get("Person").is_err());
    }

    #[test]
    fn a_syntax_error_in_a_shared_import_rejects_exactly_the_roots_reaching_it() {
        let dir = tempfile::tempdir().expect("temp dir");
        write_source(
            &dir,
            "Dep.sol",
            "pragma solidity ^0.8.0;\n\nstruct Person { address wallet;\n",
        );
        let first = write_source(
            &dir,
            "First.t.sol",
            "pragma solidity ^0.8.0;\n\nimport {Person} from \"./Dep.sol\";\n\ncontract FirstTest {}\n",
        );
        let second = write_source(
            &dir,
            "Second.t.sol",
            "pragma solidity ^0.8.0;\n\nimport {Person} from \"./Dep.sol\";\n\ncontract SecondTest {}\n",
        );
        let unaffected = write_source(
            &dir,
            "Unaffected.t.sol",
            "pragma solidity ^0.8.0;\n\ncontract UnaffectedTest {}\n",
        );

        let first_source = "project/First.t.sol";
        let second_source = "project/Second.t.sol";

        let errors = collect_with_defaults(&[
            root_at(first, first_source),
            root_at(second, second_source),
            root_at(unaffected, "project/Unaffected.t.sol"),
        ])
        .expect_err("an import that does not parse cannot be collected");

        assert_eq!(errors.len(), 2, "{errors:#?}");
        let sources: HashSet<_> = errors.iter().map(|item| item.source_name.clone()).collect();
        assert_eq!(
            sources,
            HashSet::from([PathBuf::from(first_source), PathBuf::from(second_source)])
        );
        assert!(
            errors.iter().all(|item| matches!(
                &item.problem,
                TestSourceProblem::Source(TestSourceCollectError::SourceParseErrors { reasons })
                    if reasons.iter().all(|reason| reason.contains("Dep.sol"))
            )),
            "{errors:#?}"
        );
    }

    #[test]
    fn a_root_imported_by_another_root_is_collected_from_its_retained_text() {
        let dir = tempfile::tempdir().expect("temp dir");
        let imported = write_source(
            &dir,
            "Base.t.sol",
            "pragma solidity ^0.8.0;\n\nstruct Person { address wallet; }\n\ncontract BaseTest {\n    /// forge-config: default.fuzz.runs = 7\n    function testBase(uint256 x) public {}\n}\n",
        );
        let importer = write_source(
            &dir,
            "Derived.t.sol",
            "pragma solidity ^0.8.0;\n\nimport {BaseTest} from \"./Base.t.sol\";\n\ncontract DerivedTest is BaseTest {\n    /// forge-config: default.fuzz.runs = 3\n    function testDerived(uint256 x) public {}\n}\n",
        );

        let derived_source = "project/Derived.t.sol";
        let base_source = "project/Base.t.sol";
        // Derived.t.sol is listed first, so Slang reads Base.t.sol as an import
        // before adding it as a root.
        let collected = collect_all(&[
            root_at(importer, derived_source),
            root_at(imported, base_source),
        ]);

        let base = collected_for(&collected, base_source);
        let overrides = base.overrides.get("BaseTest").expect("BaseTest overrides");
        assert_eq!(overrides.functions.len(), 1, "{:#?}", overrides.functions);
        assert_eq!(overrides.functions[0].function_name, "testBase");
        assert_eq!(
            overrides.functions[0].config.fuzz.as_ref().unwrap().runs,
            Some(7)
        );
        assert_eq!(canonical_type(base, "Person"), "Person(address wallet)");

        let derived = collected_for(&collected, derived_source);
        assert!(!derived.overrides.contains_key("BaseTest"));
        assert_eq!(
            derived
                .overrides
                .get("DerivedTest")
                .expect("DerivedTest overrides")
                .functions[0]
                .function_name,
            "testDerived"
        );
    }

    #[test]
    fn a_missing_root_fails_only_itself() {
        let dir = tempfile::tempdir().expect("temp dir");
        let present = write_source(
            &dir,
            "Present.t.sol",
            "pragma solidity ^0.8.0;\n\ncontract PresentTest {}\n",
        );

        let missing_source = "project/Missing.t.sol";

        let missing_path = dir.path().join("Missing.t.sol");

        let errors = collect_with_defaults(&[
            root_at(missing_path.clone(), missing_source),
            root_at(present, "project/Present.t.sol"),
        ])
        .expect_err("expected errors");

        assert_eq!(errors.len(), 1, "{errors:#?}");
        assert_eq!(errors[0].source_name, PathBuf::from(missing_source));
        let TestSourceProblem::Source(TestSourceCollectError::RootFileNotFound { path, .. }) =
            &errors[0].problem
        else {
            panic!("expected a missing root, got {:?}", errors[0].problem);
        };
        assert_eq!(*path, missing_path.display().to_string());
    }

    #[test]
    fn roots_of_different_versions_each_collect_a_shared_import() {
        assert_ne!(older_fixture_grammar(), fixture_grammar());

        let dir = tempfile::tempdir().expect("temp dir");
        write_source(
            &dir,
            "Shared.sol",
            "pragma solidity ^0.8.0;\n\nstruct Shared { uint256 a; }\n",
        );
        let older = write_source(
            &dir,
            "Older.t.sol",
            "pragma solidity ^0.8.0;\n\nimport {Shared} from \"./Shared.sol\";\n\ncontract OlderTest {}\n",
        );
        let newer = write_source(
            &dir,
            "Newer.t.sol",
            "pragma solidity ^0.8.0;\n\nimport {Shared} from \"./Shared.sol\";\n\ncontract NewerTest {}\n",
        );
        let older_source = "project/Older.t.sol";
        let newer_source = "project/Newer.t.sol";
        let roots = vec![
            TestSourceRoot {
                source: PathBuf::from(older_source),
                path: older,
                version: older_fixture_grammar(),
            },
            root_at(newer, newer_source),
        ];

        let collected = collect_all(&roots);

        assert_eq!(
            canonical_type(collected_for(&collected, older_source), "Shared"),
            "Shared(uint256 a)"
        );
        assert_eq!(
            canonical_type(collected_for(&collected, newer_source), "Shared"),
            "Shared(uint256 a)"
        );
    }

    #[test]
    fn an_imported_root_with_a_syntax_error_rejects_itself_and_its_importer() {
        let dir = tempfile::tempdir().expect("temp dir");
        let base = write_source(
            &dir,
            "Base.t.sol",
            "pragma solidity ^0.8.0;\n\ncontract BaseTest {\n    function testBase() public { this is not solidity\n}\n",
        );
        let derived = write_source(
            &dir,
            "Derived.t.sol",
            "pragma solidity ^0.8.0;\n\nimport {BaseTest} from \"./Base.t.sol\";\n\ncontract DerivedTest is BaseTest {}\n",
        );
        let base_source = "project/Base.t.sol";
        let derived_source = "project/Derived.t.sol";

        let errors =
            collect_with_defaults(&[root_at(base, base_source), root_at(derived, derived_source)])
                .expect_err("a root that does not parse cannot be collected");

        assert_eq!(errors.len(), 2, "{errors:#?}");
        let reasons_for = |source: &str| -> &Vec<String> {
            let item = errors
                .iter()
                .find(|item| item.source_name == Path::new(source))
                .unwrap_or_else(|| panic!("`{source}` was not rejected"));
            let TestSourceProblem::Source(TestSourceCollectError::SourceParseErrors { reasons }) =
                &item.problem
            else {
                panic!(
                    "expected parse errors for `{source}`, got {:?}",
                    item.problem
                );
            };
            reasons
        };
        // The root's own file is located by line alone, while an import names
        // its file as well.
        let in_base = format!(" in {})", file_id_of(&dir.path().join("Base.t.sol")));
        assert!(
            reasons_for(base_source)
                .iter()
                .all(|reason| reason.contains("(line ") && !reason.ends_with(&in_base)),
            "{:?}",
            reasons_for(base_source)
        );
        assert!(
            reasons_for(derived_source)
                .iter()
                .all(|reason| reason.contains("(line ") && reason.ends_with(&in_base)),
            "{:?}",
            reasons_for(derived_source)
        );
    }

    #[test]
    fn a_syntax_error_in_an_importing_root_does_not_reject_the_imported_root() {
        let dir = tempfile::tempdir().expect("temp dir");
        let base = write_source(
            &dir,
            "Base.t.sol",
            "pragma solidity ^0.8.0;\n\ncontract BaseTest {}\n",
        );
        let derived = write_source(
            &dir,
            "Derived.t.sol",
            "pragma solidity ^0.8.0;\n\nimport {BaseTest} from \"./Base.t.sol\";\n\ncontract DerivedTest is BaseTest {\n    function testDerived() public { this is not solidity\n}\n",
        );
        let derived_source = "project/Derived.t.sol";

        let errors = collect_with_defaults(&[
            root_at(base, "project/Base.t.sol"),
            root_at(derived, derived_source),
        ])
        .expect_err("the importing root does not parse");

        assert_eq!(errors.len(), 1, "{errors:#?}");
        assert_eq!(errors[0].source_name, PathBuf::from(derived_source));
    }

    #[test]
    fn parse_error_reasons_are_truncated_with_a_count_of_the_rest() {
        const UNREPORTED_IMPORTS: usize = 3;

        let dir = tempfile::tempdir().expect("temp dir");
        // Each import is truncated mid-struct, which Slang recovers from with
        // one diagnostic, so each contributes one reason.
        let broken_import_count = MAX_REPORTED_PARSE_ERRORS + UNREPORTED_IMPORTS;
        let imports: String = (0..broken_import_count)
            .map(|index| {
                write_source(
                    &dir,
                    &format!("Broken{index}.sol"),
                    "pragma solidity ^0.8.0;\n\nstruct Broken { uint256 a;\n",
                );
                format!("import \"./Broken{index}.sol\";\n")
            })
            .collect();
        let root = write_source(
            &dir,
            "Importer.t.sol",
            &format!("pragma solidity ^0.8.0;\n\n{imports}\ncontract ImporterTest {{}}\n"),
        );

        let errors = collect_one(root_at(root, FIXTURE_SOURCE)).expect_err("expected errors");

        let TestSourceProblem::Source(TestSourceCollectError::SourceParseErrors { reasons }) =
            &errors[0].problem
        else {
            panic!("expected parse errors, got {:?}", errors[0].problem);
        };
        assert_eq!(reasons.len(), MAX_REPORTED_PARSE_ERRORS + 1, "{reasons:#?}");
        assert!(reasons[0].contains("Broken"), "{reasons:#?}");
        assert_eq!(
            reasons[MAX_REPORTED_PARSE_ERRORS],
            format!("and {UNREPORTED_IMPORTS} more")
        );
    }

    #[test]
    fn two_source_names_for_one_file_are_both_collected() {
        let dir = tempfile::tempdir().expect("temp dir");
        let shared = write_source(
            &dir,
            "Shared.t.sol",
            "pragma solidity ^0.8.0;\n\nstruct Person { address wallet; }\n\ncontract SharedTest {}\n",
        );
        let first_source = "project/First.t.sol";
        let second_source = "project/Second.t.sol";

        let collected = collect_all(&[
            root_at(shared.clone(), first_source),
            root_at(shared, second_source),
        ]);

        assert_eq!(
            canonical_type(collected_for(&collected, first_source), "Person"),
            "Person(address wallet)"
        );
        assert_eq!(
            canonical_type(collected_for(&collected, second_source), "Person"),
            "Person(address wallet)"
        );
    }

    #[test]
    fn two_source_names_for_one_missing_file_are_both_reported() {
        let dir = tempfile::tempdir().expect("temp dir");
        let missing = dir.path().join("Missing.t.sol");

        let errors = collect_with_defaults(&[
            root_at(missing.clone(), "project/First.t.sol"),
            root_at(missing, "project/Second.t.sol"),
        ])
        .expect_err("expected errors");

        let sources: Vec<_> = errors.iter().map(|item| item.source_name.clone()).collect();
        assert_eq!(
            sources,
            vec![
                PathBuf::from("project/First.t.sol"),
                PathBuf::from("project/Second.t.sol"),
            ]
        );
        assert!(errors.iter().all(|item| matches!(
            &item.problem,
            TestSourceProblem::Source(TestSourceCollectError::RootFileNotFound { .. })
        )));
    }

    #[test]
    fn an_unresolvable_import_narrows_the_closure_without_rejecting_the_root() {
        let dir = tempfile::tempdir().expect("temp dir");
        let root = write_source(
            &dir,
            "Unmapped.t.sol",
            "pragma solidity ^0.8.0;\n\nimport {Person} from \"@dep/Person.sol\";\n\ncontract UnmappedTest {}\n",
        );

        let collected = unwrap_collected(collect_one(root_at(root, FIXTURE_SOURCE)));

        assert!(collected.eip712_types.is_empty());
    }

    #[test]
    fn problems_are_reported_in_source_order() {
        let dir = tempfile::tempdir().expect("temp dir");
        let roots: Vec<TestSourceRoot> = ["Zeta", "Alpha", "Mid"]
            .into_iter()
            .map(|name| {
                root_at(
                    dir.path().join(format!("{name}.t.sol")),
                    &format!("project/{name}.t.sol"),
                )
            })
            .collect();

        let errors = collect_with_defaults(&roots).expect_err("every root is missing");

        let sources: Vec<_> = errors.iter().map(|item| item.source_name.clone()).collect();
        assert_eq!(
            sources,
            vec![
                PathBuf::from("project/Alpha.t.sol"),
                PathBuf::from("project/Mid.t.sol"),
                PathBuf::from("project/Zeta.t.sol"),
            ]
        );
    }
}
