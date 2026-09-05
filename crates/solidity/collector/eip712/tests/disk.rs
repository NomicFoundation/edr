//! Disk-based tests for [`collect_eip712_types_from_compilation_unit`],
//! exercising the on-disk file reading and import resolution that the
//! in-memory unit tests in the crate cannot.
//!
//! These build the compilation unit the same way the test runner does, via
//! [`build_compilation_unit`].

use std::{collections::HashMap, path::PathBuf};

use edr_solidity_collector_eip712::{
    collector::{collect_eip712_types_from_compilation_unit, Eip712TypeCollection},
    ImportResolver,
};
use edr_solidity_parser_slang::{build_compilation_unit, language_version_for_solc};
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
    let unit = build_compilation_unit(&root, language_version, import_resolver);

    collect_eip712_types_from_compilation_unit(&unit, &root.to_string_lossy())
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
    // No import mapping supplied: the import is unresolved (a diagnostic, not a
    // hard error). `Payment` depends on the missing `Token`, so it is not
    // usable, but collection itself still succeeds.
    let types = collect("mapped/Root.sol", solc(), &ImportResolver::default());

    assert!(types.get("Token").is_err());
}

#[test]
fn missing_root_file_yields_no_types() {
    // A build over a missing root yields a diagnostic and an empty unit; the
    // test runner pre-checks the root's existence to tell this apart from a
    // source that genuinely declares no structs.
    let types = collect("does/not/exist.sol", solc(), &ImportResolver::default());

    assert!(types.is_empty());
}
