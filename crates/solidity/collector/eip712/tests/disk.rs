//! Disk-based tests for [`UnitStructs`], exercising the on-disk file reading
//! and import resolution that the in-memory unit tests in the crate cannot.
//!
//! Collection is scoped to the files the root reaches, as the test runner
//! scopes it.

use std::{collections::HashMap, path::PathBuf};

use edr_solidity_collector_eip712::{
    collector::{Eip712TypeCollection, UnitStructs},
    ImportResolver,
};
use edr_solidity_parser_slang::compilation::{
    build_multi_root_compilation_unit, file_id_of, language_version_for_solc, MultiRootCompilation,
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
/// types, mirroring what `edr_solidity_tests::test_sources` does per root.
fn collect(
    relative: &str,
    version: Version,
    import_resolver: &ImportResolver,
) -> Eip712TypeCollection {
    let root = fixture(relative);
    let language_version = language_version_for_solc(&version).expect("supported solc version");
    let MultiRootCompilation { unit, sources } =
        build_multi_root_compilation_unit([root.as_path()], language_version, import_resolver);
    let root_id = file_id_of(&root);

    UnitStructs::from_unit(&unit).types_for(&root_id, &sources.reachable_from(&root_id))
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
