//! Disk-based tests for [`CachedEip712Provider`], exercising the on-disk file
//! reading and import resolution that the in-memory unit tests in the crate
//! cannot.

use std::{
    collections::HashMap,
    path::{Path, PathBuf},
};

use edr_solidity_collector_eip712::{
    collector::Eip712CollectError,
    provider::{CachedEip712TypeProvider, Eip712Root},
    ImportResolver,
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

/// A root whose query identity (`source`) is the relative fixture path and
/// whose on-disk `path` is the absolute location.
fn root(relative: &str, version: &Version) -> Eip712Root {
    Eip712Root {
        source: PathBuf::from(relative),
        path: fixture(relative),
        version: version.clone(),
    }
}

#[test]
fn resolves_relative_imports() {
    let provider = CachedEip712TypeProvider::collect(
        &[root("relative/Root.sol", &solc())],
        &ImportResolver::default(),
    )
    .expect("collection should succeed");

    assert_eq!(
        provider
            .get_eip712_type(Path::new("relative/Root.sol"), "Mail")
            .unwrap()
            .canonical_definition(),
        "Mail(Person from,Person to,string contents)Person(address wallet,string name)"
    );
}

#[test]
fn resolves_mapped_imports() {
    let mut import_map = HashMap::new();
    import_map.insert(
        "@lib/Token.sol".to_string(),
        fixture("mapped/lib/Token.sol"),
    );

    let provider = CachedEip712TypeProvider::collect(
        &[root("mapped/Root.sol", &solc())],
        &ImportResolver::new(import_map),
    )
    .expect("collection should succeed");

    assert_eq!(
        provider
            .get_eip712_type(Path::new("mapped/Root.sol"), "Payment")
            .unwrap()
            .canonical_definition(),
        "Payment(Token token,uint256 amount)Token(address addr,uint8 decimals)"
    );
}

#[test]
fn unmapped_import_leaves_dependency_unresolved_but_unit_builds() {
    // No import mapping supplied: the import is unresolved (a diagnostic, not a
    // hard error). `Payment` depends on the missing `Token`, so it is not
    // usable, but collection itself still succeeds.
    let provider = CachedEip712TypeProvider::collect(
        &[root("mapped/Root.sol", &solc())],
        &ImportResolver::default(),
    )
    .expect("collection should still succeed despite the unresolved import");

    assert!(provider
        .get_eip712_type(Path::new("mapped/Root.sol"), "Token")
        .is_err());
}

#[test]
fn missing_root_file_is_an_error() {
    let error = CachedEip712TypeProvider::collect(
        &[root("does/not/exist.sol", &solc())],
        &ImportResolver::default(),
    )
    .unwrap_err();
    assert!(matches!(error, Eip712CollectError::RootFileNotFound { .. }));
}

#[test]
fn unsupported_solc_version_is_an_error() {
    let error = CachedEip712TypeProvider::collect(
        &[root("relative/Root.sol", &Version::new(0, 7, 6))],
        &ImportResolver::default(),
    )
    .unwrap_err();
    assert!(matches!(
        error,
        Eip712CollectError::InvalidSolcVersion { .. }
    ));
}
