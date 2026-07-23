//! Combined collection over the test sources.
//!
//! Each unique test source is read from disk and parsed with Slang exactly
//! once; both its inline test configuration (`forge-config:`/
//! `hardhat-config:` NatSpec directives) and its EIP-712 struct definitions
//! (served to the `eip712HashType`/`eip712HashStruct` cheatcodes) are
//! extracted from that same compilation unit. The unit is dropped afterwards
//! — nothing is cached beyond the extracted data.

use std::{collections::HashMap, path::PathBuf, sync::Arc};

use edr_solidity_collector_eip712::collector::{
    collect_eip712_types_from_compilation_unit, Eip712TypeCollection,
};
use edr_solidity_parser_slang::{build_compilation_unit, ImportResolver};
use rayon::prelude::*;
use semver::Version;

use crate::inline_config::{
    collect_source_from_unit, InlineConfigCollectError, InlineConfigErrorItem,
    InlineConfigProblem, SourceOverrides,
};

/// A Solidity test source to collect from.
#[derive(Clone, Debug)]
pub(crate) struct TestSourceRoot {
    /// The identity this root is queried by — the compiled artifact's solc
    /// source name (e.g. the running test contract's `source`). Collections
    /// are keyed by this, because that is what a query has in hand.
    pub source: PathBuf,
    /// Absolute path to the file on disk, used to read and parse it.
    pub path: PathBuf,
    /// The solc version the file was compiled with.
    pub version: Version,
}

/// Everything collected from one test source's single parse.
#[derive(Default)]
pub(crate) struct CollectedTestSource {
    /// The successfully-parsed inline configuration, keyed by contract name.
    pub overrides: SourceOverrides,
    /// The EIP-712 struct definitions reachable from the source.
    pub eip712_types: Eip712TypeCollection,
}

/// Reads and parses every root in parallel, extracting both collections from
/// each root's single compilation unit.
///
/// Problems (an unreadable root file, an unsupported solc version, or a
/// malformed directive) are accumulated — in root order, each located at its
/// source — rather than short-circuited, so every problem across every source
/// is reported together; callers abort the run if any were found. A root that
/// failed to read or parse contributes empty collections.
pub(crate) fn collect_test_sources(
    roots: &[TestSourceRoot],
    import_resolver: &ImportResolver,
) -> (
    HashMap<PathBuf, CollectedTestSource>,
    Vec<InlineConfigErrorItem>,
) {
    // The roots are parsed in parallel on rayon's global pool. Collection runs
    // synchronously and completes before any test suite is dispatched, so it
    // never contends with suite execution.
    let collected: Vec<_> = roots
        .par_iter()
        .map(|root| collect_root(root, import_resolver))
        .collect();

    let mut by_source = HashMap::with_capacity(collected.len());
    let mut errors = Vec::new();
    for (source, collected_source, source_errors) in collected {
        errors.extend(source_errors);
        by_source.insert(source, collected_source);
    }

    (by_source, errors)
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
) -> (PathBuf, CollectedTestSource, Vec<InlineConfigErrorItem>) {
    let source_error = |error: InlineConfigCollectError| {
        (
            root.source.clone(),
            CollectedTestSource::default(),
            vec![InlineConfigErrorItem {
                source: root.source.clone(),
                problem: InlineConfigProblem::Source(error),
            }],
        )
    };

    // Read the content up front: the NatSpec directives are recovered from the
    // raw source text, and a build over a missing root only yields a
    // diagnostic and an empty unit, which must not be mistaken for "no types".
    let content = match std::fs::read_to_string(&root.path) {
        Ok(content) => content,
        Err(error) => {
            return source_error(InlineConfigCollectError::RootFileNotFound {
                path: root.path.display().to_string(),
                reason: error.to_string(),
            });
        }
    };

    let unit = match build_compilation_unit(&root.path, root.version.clone(), import_resolver) {
        Ok(unit) => unit,
        Err(error) => return source_error(error.into()),
    };

    let file_id = root.path.to_string_lossy();
    let collection = collect_source_from_unit(&root.source, Arc::from(content), &unit, &file_id);
    let eip712_types = collect_eip712_types_from_compilation_unit(&unit);

    (
        root.source.clone(),
        CollectedTestSource {
            overrides: collection.overrides,
            eip712_types,
        },
        collection.errors,
    )
}

#[cfg(test)]
mod tests {
    use std::io::Write as _;

    use super::*;

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
            version,
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

        let (by_source, errors) =
            collect_test_sources(&[root], &ImportResolver::default());

        assert!(errors.is_empty(), "unexpected errors: {errors:?}");
        let collected = &by_source[&PathBuf::from("project/C.t.sol")];

        let overrides = collected.overrides.get("C").expect("C has overrides");
        assert_eq!(overrides.len(), 1);
        assert_eq!(overrides[0].function_name, "testFoo");

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

        let (by_source, errors) =
            collect_test_sources(&[root], &ImportResolver::default());

        assert!(errors.is_empty(), "unexpected errors: {errors:?}");
        let collected = &by_source[&PathBuf::from("project/C.t.sol")];
        assert!(collected.overrides.is_empty());
        assert!(collected.eip712_types.get("Person").is_err());
    }

    #[test]
    fn unsupported_solc_version_is_a_source_error() {
        let file = temp_source("contract C {}");
        let root = root_for(&file, "project/C.t.sol", Version::new(0, 7, 6));

        let (by_source, errors) =
            collect_test_sources(&[root], &ImportResolver::default());

        assert_eq!(errors.len(), 1);
        assert!(matches!(
            &errors[0].problem,
            InlineConfigProblem::Source(InlineConfigCollectError::InvalidSolcVersion(_))
        ));
        // The source still gets (empty) collections.
        let collected = &by_source[&PathBuf::from("project/C.t.sol")];
        assert!(collected.overrides.is_empty());
    }

    #[test]
    fn missing_root_file_is_a_source_error() {
        let root = TestSourceRoot {
            source: PathBuf::from("project/Missing.t.sol"),
            path: PathBuf::from("/does/not/exist/Missing.t.sol"),
            version: Version::new(0, 8, 24),
        };

        let (_, errors) = collect_test_sources(&[root], &ImportResolver::default());

        assert_eq!(errors.len(), 1);
        assert!(matches!(
            &errors[0].problem,
            InlineConfigProblem::Source(InlineConfigCollectError::RootFileNotFound { .. })
        ));
    }
}
