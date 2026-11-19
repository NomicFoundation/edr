//! Disk-based tests for [`UnitStructs`], exercising the on-disk file reading
//! and import resolution that the in-memory unit tests in the crate cannot.
//!
//! Collection is scoped to the files the root reaches, as the test runner
//! scopes it.

use std::{
    collections::{HashMap, HashSet},
    path::PathBuf,
};

use edr_solidity_collector_eip712::collector::{
    Eip712CollectionLookupError, Eip712TypeCollection, Eip712TypeRejected, RejectReason,
    UnitStructs,
};
use edr_solidity_parser_slang::{
    compilation::{
        build_multi_root_compilation_unit, file_id_of, language_version_for_solc,
        MultiRootCompilation,
    },
    resolver::ImportResolver,
};
use semver::Version;

fn fixture(relative: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(relative)
}

fn solc() -> Version {
    Version::new(0, 8, 24)
}

/// Builds the fixture's compilation unit from disk and collects its EIP-712
/// types from every reachable file but `excluded`, mirroring what
/// `edr_solidity_tests::test_sources` does per root.
fn collect_excluding(
    relative: &str,
    version: Version,
    import_resolver: &ImportResolver,
    excluded: &[&str],
) -> Eip712TypeCollection {
    let root = fixture(relative);
    let language_version = language_version_for_solc(&version).expect("supported solc version");
    let MultiRootCompilation { unit, sources } =
        build_multi_root_compilation_unit([root.as_path()], language_version, import_resolver);
    let root_id = file_id_of(&root);

    let excluded_ids: Vec<String> = excluded
        .iter()
        .map(|relative| file_id_of(&fixture(relative)))
        .collect();
    let reachable = sources.reachable_from(&root_id);
    let (excluded_from, collected_from): (HashSet<&str>, HashSet<&str>) = reachable
        .iter()
        .copied()
        .partition(|reached| excluded_ids.iter().any(|excluded| excluded == reached));

    UnitStructs::from_unit(&unit).types_for(&root_id, &collected_from, &excluded_from)
}

/// Collects the EIP-712 types of every file the fixture's root reaches.
fn collect(
    relative: &str,
    version: Version,
    import_resolver: &ImportResolver,
) -> Eip712TypeCollection {
    collect_excluding(relative, version, import_resolver, &[])
}

#[test]
fn resolves_relative_imports() {
    let types = collect("relative/Root.sol", solc(), &ImportResolver::default());

    assert_eq!(
        types.get("Mail").unwrap().canonical_definition(),
        "Mail(Person from,Person to,string contents)Person(address wallet,string name)"
    );
}

#[test]
fn resolves_mapped_imports() {
    let import_map = HashMap::from([(
        "@lib/Token.sol".to_string(),
        fixture("mapped/lib/Token.sol"),
    )]);

    let types = collect("mapped/Root.sol", solc(), &ImportResolver::new(import_map));

    assert_eq!(
        types.get("Payment").unwrap().canonical_definition(),
        "Payment(Token token,uint256 amount)Token(address addr,uint8 decimals)"
    );
}

#[test]
fn unmapped_import_leaves_dependency_unresolved_but_unit_builds() {
    // No import mapping is supplied, so the import is unresolved (a
    // diagnostic, not a hard error). Collection still succeeds, but
    // `Payment`'s member type does not resolve. `Payment` is therefore
    // rejected rather than encoded without it.
    let types = collect("mapped/Root.sol", solc(), &ImportResolver::default());

    assert!(types.get("Token").is_err());

    let error = types
        .get("Payment")
        .expect_err("a member behind an unresolved import cannot be encoded");
    assert!(error.to_string().contains("token"), "{error}");
}

/// A member keeps the declaration it resolves to, so a same-named struct in an
/// excluded file is not silently replaced by the collected one. Inlining the
/// collected body would hash a type the source does not declare.
#[test]
fn dependency_in_an_excluded_file_rejects_its_dependents() {
    /// The sole dependency a rejection names, as "`Name` in <file>".
    fn excluded_dependency(error: Eip712CollectionLookupError) -> String {
        let Eip712CollectionLookupError::Rejected(Eip712TypeRejected {
            reason: RejectReason::DependenciesInExcludedFiles { mut dependencies },
            ..
        }) = error
        else {
            panic!("expected a dependency in an excluded file: {error}");
        };

        assert_eq!(dependencies.len(), 1, "{dependencies:?}");
        dependencies.remove(0)
    }

    let types = collect_excluding(
        "excluded/Root.sol",
        solc(),
        &ImportResolver::default(),
        &["excluded/B.sol", "excluded/E.sol"],
    );

    assert_eq!(
        types
            .get("S")
            .expect("A.sol declares it")
            .canonical_definition(),
        "S(uint256 a)"
    );

    // `Wrapper.s` is B.sol's `S`, not the collected `S` of A.sol.
    let wrapper = types
        .get("Wrapper")
        .expect_err("a member from an excluded file cannot be inlined by name");
    let wrapper = excluded_dependency(wrapper);
    assert!(wrapper.starts_with("`S` in "), "{wrapper}");
    assert!(wrapper.ends_with("B.sol"), "{wrapper}");

    // `T.inner` is E.sol's `T`, so it is a dependency rather than the
    // self-reference its bare name suggests.
    let recursive = types
        .get("T")
        .expect_err("a same-named member from an excluded file is no self-reference");
    let recursive = excluded_dependency(recursive);
    assert!(recursive.starts_with("`T` in "), "{recursive}");
    assert!(recursive.ends_with("E.sol"), "{recursive}");
}
