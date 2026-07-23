//! Inline-config resolution.
//!
//! Collection: given the set of test sources to cover — each with the absolute
//! path of its file on disk — parse each with Slang and extract every
//! contract's inline configuration, accumulating every problem found (a
//! malformed directive, an unreadable root file, or an unsupported solc
//! version).
//!
//! Use: the runner first [`validate`](SharedInlineConfigProvider::validate)s —
//! if collection found any problem, runner creation fails and the whole run is
//! aborted before any test executes (matching Hardhat/Foundry). Otherwise each
//! suite [`get`](SharedInlineConfigProvider::get)s the precomputed
//! configuration of its test contract — a plain map lookup, since all parsing
//! happened during collection.

use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::Arc,
};

use rayon::prelude::*;
use semver::Version;

use edr_solidity_parser_slang::ImportResolver;

use super::{
    directives,
    error::{
        InlineConfigCollectError, InlineConfigErrorItem, InlineConfigErrors, InlineConfigProblem,
    },
    overrides::{collect_source, ContractInlineConfig, SourceCollection, SourceOverrides},
};

/// A Solidity test source to collect inline configuration from.
#[derive(Clone, Debug)]
pub struct InlineConfigRoot {
    /// The identity this root is queried by — the compiled artifact's solc
    /// source name (e.g. the running test contract's `source`). Collections are
    /// keyed by this, because that is what a query has in hand.
    pub source: PathBuf,
    /// Absolute path to the file on disk, used to read and parse it.
    pub path: PathBuf,
    /// The solc version the file was compiled with.
    pub version: Version,
}

/// Caches the fully-parsed inline configuration of every test contract, keyed
/// by source name, so a query is a plain lookup, and the problems found while
/// parsing so the run can be aborted up front.
///
/// [`collect`](Self::collect) does all the work — read each source and its
/// imports from disk, parse them with Slang, and extract every contract's
/// inline configuration — once. Only sources that carry a directive are
/// parsed. Problems (a malformed directive, an unreadable root file, or an
/// unsupported solc version) are accumulated (see
/// [`validate`](Self::validate)) rather than short-circuiting, so every
/// problem across every source is reported together.
#[derive(Debug)]
pub struct CachedInlineConfigProvider {
    by_source: HashMap<PathBuf, SourceOverrides>,
    errors: Vec<InlineConfigErrorItem>,
}

impl CachedInlineConfigProvider {
    /// Parses every root's inline configuration in parallel, reading each
    /// root's file — and its imports, resolved by `import_resolver` — from
    /// disk. Sources that carry no inline-config directive are skipped. Every
    /// problem found (a malformed directive, an unreadable root file, or an
    /// unsupported solc version) is accumulated and surfaced by
    /// [`validate`](Self::validate).
    pub fn collect(roots: &[InlineConfigRoot], import_resolver: &ImportResolver) -> Self {
        let parse = |root: &InlineConfigRoot| -> Option<(PathBuf, SourceCollection)> {
            let content = match std::fs::read_to_string(&root.path) {
                Ok(content) => content,
                Err(error) => {
                    return Some((
                        root.source.clone(),
                        SourceCollection {
                            overrides: SourceOverrides::new(),
                            errors: vec![InlineConfigErrorItem {
                                source: root.source.clone(),
                                problem: InlineConfigProblem::Source(
                                    InlineConfigCollectError::RootFileNotFound {
                                        path: root.path.display().to_string(),
                                        reason: error.to_string(),
                                    },
                                ),
                            }],
                        },
                    ));
                }
            };
            // Fast path: only parse sources that carry a directive.
            if !directives::contains_inline_config_directive(&content) {
                return None;
            }
            let collection = collect_source(
                &root.source,
                &root.path,
                &content,
                root.version.clone(),
                import_resolver,
            );
            Some((root.source.clone(), collection))
        };

        // Parse the roots in parallel on rayon's global pool. Collection runs
        // synchronously and completes before any test suite is dispatched, so it
        // never contends with suite execution.
        let collected: Vec<Option<(PathBuf, SourceCollection)>> =
            roots.par_iter().map(parse).collect();

        let mut by_source = HashMap::new();
        let mut errors = Vec::new();
        for (source, collection) in collected.into_iter().flatten() {
            let SourceCollection {
                overrides,
                errors: source_errors,
            } = collection;
            // Each item already carries its source name, contract, function and
            // line, so problems from every source flatten into one report.
            errors.extend(source_errors);
            if !overrides.is_empty() {
                by_source.insert(source, overrides);
            }
        }

        Self { by_source, errors }
    }

    /// Returns `Err` listing every problem found during collection, or `Ok` if
    /// there were none. Callers abort the run on `Err` before any suite runs.
    pub fn validate(&self) -> Result<(), InlineConfigErrors> {
        if self.errors.is_empty() {
            Ok(())
        } else {
            Err(InlineConfigErrors::new(self.errors.clone()))
        }
    }

    /// Returns the inline configuration of `contract_name` within `source`, as
    /// computed during collection: its contract-level configuration and the
    /// overrides of every test function declared directly in it.
    ///
    /// Returns an empty configuration if the contract carries none. Malformed
    /// directives never reach here — they are caught up front by
    /// [`validate`](Self::validate), which aborts the run.
    pub fn get(&self, source: &Path, contract_name: &str) -> ContractInlineConfig {
        self.by_source
            .get(source)
            .and_then(|configs| configs.get(contract_name))
            .cloned()
            .unwrap_or_default()
    }
}

/// A cloneable, `Send + Sync` handle to the collected inline configuration,
/// shared across the test runner's parallel suite dispatch.
#[derive(Clone, Debug)]
pub struct SharedInlineConfigProvider(Arc<CachedInlineConfigProvider>);

impl SharedInlineConfigProvider {
    /// Collects every root's inline configuration, returning a shared handle to
    /// the result. Problems found during collection are surfaced together by
    /// [`validate`](Self::validate), which the runner calls up front to abort
    /// the run before any test executes.
    pub fn collect(roots: Vec<InlineConfigRoot>, import_resolver: ImportResolver) -> Self {
        Self(Arc::new(CachedInlineConfigProvider::collect(
            &roots,
            &import_resolver,
        )))
    }

    /// Returns `Err` listing every problem found during collection, or `Ok` if
    /// there were none. The runner calls this before dispatching any suite and
    /// aborts on `Err`.
    pub fn validate(&self) -> Result<(), InlineConfigErrors> {
        self.0.validate()
    }

    /// Returns the inline configuration of `contract_name` within `source` — a
    /// plain lookup against the already-collected configuration.
    pub fn get(&self, source: &Path, contract_name: &str) -> ContractInlineConfig {
        self.0.get(source, contract_name)
    }
}

#[cfg(test)]
mod tests {
    use std::{io::Write as _, thread};

    use super::*;

    const SOURCE_NAME: &str = "project/test.sol";
    const SOURCE: &str = r#"// SPDX-License-Identifier: MIT
pragma solidity ^0.8.0;

/// forge-config: default.invariant.runs = 7
contract MyTest {
    uint256 internal value;

    /// forge-config: default.fuzz.runs = 42
    function testFuzz(uint256 x) public { value = x; }

    /// hardhat-config: isolate = true
    function testUnit() public {}

    // not natspec: forge-config: default.fuzz.runs = 999
    function testNoConfig() public {}
}
"#;

    /// A source with two malformed test functions — one of which has *two* bad
    /// directives — and one well-formed function, to pin down that exactly one
    /// error is reported per malformed function (not per source, and not one
    /// per bad directive).
    const MALFORMED_SOURCE: &str = r#"// SPDX-License-Identifier: MIT
pragma solidity ^0.8.0;

contract BadTest {
    /// forge-config: default.fuzz.runs = -1
    /// forge-config: fuzz.maxTestRejects = -2
    function testFuzz(uint256 x) public {}

    /// forge-config: fuzz.bogus = 1
    function testOther() public {}

    /// forge-config: default.fuzz.runs = 5
    function testValid(uint256 x) public {}
}
"#;

    /// Writes `source` to a temporary `.sol` file and returns a root under the
    /// query identity `source_name`, together with the handle keeping the
    /// file alive.
    fn root_named(source_name: &str, source: &str) -> (InlineConfigRoot, tempfile::NamedTempFile) {
        let mut file = tempfile::Builder::new()
            .suffix(".sol")
            .tempfile()
            .expect("temp file");
        file.write_all(source.as_bytes()).expect("write source");

        let root = InlineConfigRoot {
            source: PathBuf::from(source_name),
            path: file.path().to_path_buf(),
            version: Version::new(0, 8, 0),
        };
        (root, file)
    }

    fn root_with_source() -> (InlineConfigRoot, tempfile::NamedTempFile) {
        root_named(SOURCE_NAME, SOURCE)
    }

    fn malformed_root() -> (InlineConfigRoot, tempfile::NamedTempFile) {
        root_named("project/bad.sol", MALFORMED_SOURCE)
    }

    fn assert_overrides(config: &ContractInlineConfig) {
        let contract = config.contract.as_ref().expect("contract-level config");
        assert_eq!(contract.invariant.as_ref().unwrap().runs, Some(7));

        let overrides = &config.functions;
        assert_eq!(overrides.len(), 2, "{overrides:#?}");
        let fuzz = overrides
            .iter()
            .find(|o| o.function_name == "testFuzz")
            .expect("testFuzz");
        assert_eq!(fuzz.config.fuzz.as_ref().unwrap().runs, Some(42));
        let unit = overrides
            .iter()
            .find(|o| o.function_name == "testUnit")
            .expect("testUnit");
        assert_eq!(unit.config.isolate, Some(true));
        assert!(overrides.iter().all(|o| o.function_name != "testNoConfig"));
    }

    #[test]
    fn cached_collects_and_queries() {
        let (root, _file) = root_with_source();
        let provider = CachedInlineConfigProvider::collect(&[root], &ImportResolver::default());

        provider.validate().expect("no problems");
        assert_overrides(&provider.get(Path::new(SOURCE_NAME), "MyTest"));
        // A source that was never collected reports no configuration.
        assert!(provider.get(Path::new("never.sol"), "MyTest").is_empty());
    }

    #[test]
    fn missing_root_file_reported_by_validate() {
        let root = InlineConfigRoot {
            source: PathBuf::from(SOURCE_NAME),
            path: PathBuf::from("/nonexistent/test.sol"),
            version: Version::new(0, 8, 0),
        };

        let provider = CachedInlineConfigProvider::collect(&[root], &ImportResolver::default());
        let errors = provider.validate().expect_err("problems reported");
        let items = errors.items();
        assert_eq!(items.len(), 1, "{items:#?}");
        // A source-level problem has no directive to point at.
        assert_eq!(items[0].source, PathBuf::from(SOURCE_NAME));
        let InlineConfigProblem::Source(error) = &items[0].problem else {
            panic!(
                "expected a source-level problem, got {:#?}",
                items[0].problem
            );
        };
        assert!(matches!(
            error,
            InlineConfigCollectError::RootFileNotFound { .. }
        ));
        assert!(error.to_string().contains("/nonexistent/test.sol"));
    }

    #[test]
    fn cached_accumulates_one_error_per_function() {
        let (root, _file) = malformed_root();
        let provider = CachedInlineConfigProvider::collect(&[root], &ImportResolver::default());

        let errors = provider.validate().expect_err("problems reported");
        let items = errors.items();

        // Exactly one problem per malformed function — not one per source, and
        // not one per bad directive (`testFuzz` has two), and never the
        // well-formed `testValid`.
        assert_eq!(items.len(), 2, "{items:#?}");

        // Each item carries structured location: source, contract, function and
        // the 1-based line of the *first* offending directive in that function.
        let fuzz = items
            .iter()
            .find(|item| {
                matches!(
                    &item.problem,
                    InlineConfigProblem::Directive { function, .. } if function.as_deref() == Some("testFuzz")
                )
            })
            .expect("testFuzz reported");
        assert_eq!(fuzz.source, PathBuf::from("project/bad.sol"));
        let InlineConfigProblem::Directive { contract, line, .. } = &fuzz.problem else {
            unreachable!("filtered to a testFuzz directive above");
        };
        assert_eq!(contract, "BadTest");
        assert_eq!(*line, 5); // the `runs = -1` line, not `-2` on line 6

        let other = items
            .iter()
            .find(|item| {
                matches!(
                    &item.problem,
                    InlineConfigProblem::Directive { function, .. } if function.as_deref() == Some("testOther")
                )
            })
            .expect("testOther reported");
        let InlineConfigProblem::Directive { line, .. } = &other.problem else {
            unreachable!("filtered to a testOther directive above");
        };
        assert_eq!(*line, 9);

        assert!(items.iter().all(|item| {
            !matches!(
                &item.problem,
                InlineConfigProblem::Directive { function, .. } if function.as_deref() == Some("testValid")
            )
        }));

        // The well-formed function's override is still collected alongside the
        // malformed ones.
        let config = provider.get(Path::new("project/bad.sol"), "BadTest");
        assert_eq!(config.functions.len(), 1);
        assert_eq!(config.functions[0].function_name, "testValid");
    }

    /// A malformed contract-level directive is reported against the contract
    /// (no function) at the offending line, while a well-formed function-level
    /// override in the same contract is still collected.
    #[test]
    fn malformed_contract_level_directive_reported_without_function() {
        let source = r#"// SPDX-License-Identifier: MIT
pragma solidity ^0.8.0;

/// forge-config: default.fuzz.runs = -1
contract BadContractLevel {
    /// forge-config: default.fuzz.runs = 5
    function testValid(uint256 x) public {}
}
"#;
        let (root, _file) = root_named("project/bad_contract.sol", source);
        let provider = CachedInlineConfigProvider::collect(&[root], &ImportResolver::default());

        let errors = provider.validate().expect_err("problems reported");
        let items = errors.items();
        assert_eq!(items.len(), 1, "{items:#?}");
        let InlineConfigProblem::Directive {
            contract,
            function,
            line,
            ..
        } = &items[0].problem
        else {
            panic!("expected a directive problem, got {:#?}", items[0].problem);
        };
        assert_eq!(contract, "BadContractLevel");
        assert_eq!(*function, None);
        assert_eq!(*line, 4);

        // The rendered report names the contract without a function.
        assert!(
            items[0].to_string().contains("BadContractLevel:"),
            "{}",
            items[0]
        );

        let config = provider.get(Path::new("project/bad_contract.sol"), "BadContractLevel");
        assert!(config.contract.is_none());
        assert_eq!(config.functions.len(), 1);
        assert_eq!(config.functions[0].function_name, "testValid");
    }

    #[test]
    fn shared_serves_concurrent_queries() {
        let (root, _file) = root_with_source();
        let provider = SharedInlineConfigProvider::collect(vec![root], ImportResolver::default());

        provider.validate().expect("no problems");

        let handles: Vec<_> = (0..8)
            .map(|_| {
                let provider = provider.clone();
                thread::spawn(move || provider.get(Path::new(SOURCE_NAME), "MyTest"))
            })
            .collect();

        for handle in handles {
            assert_overrides(&handle.join().unwrap());
        }
    }

    #[test]
    fn shared_validate_reports_collection_problems() {
        let (good, _good_file) = root_with_source();
        let (bad, _bad_file) = malformed_root();
        let provider =
            SharedInlineConfigProvider::collect(vec![good, bad], ImportResolver::default());

        let errors = provider.validate().expect_err("problems reported");
        assert!(errors.to_string().contains("project/bad.sol"));
        // The well-formed source is still queryable alongside the bad one.
        assert_overrides(&provider.get(Path::new(SOURCE_NAME), "MyTest"));
    }
}
