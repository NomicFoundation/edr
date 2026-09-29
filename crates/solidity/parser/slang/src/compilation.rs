//! Building Slang compilation units over on-disk Solidity sources.

use std::path::Path;

use semver::Version;
use slang_solidity_v2::{
    compilation::{CompilationBuilder, CompilationUnit},
    utils::LanguageVersion,
};

use crate::resolver::{ImportResolver, SourceProvider};

/// Maps a solc version to the Slang grammar that parses it, or `None` when it
/// predates the oldest grammar Slang ships.
///
/// A version newer than the newest grammar clamps down to it, and build and
/// pre-release metadata is ignored: a nightly parses as its release.
pub fn language_version_for_solc(solc_version: &Version) -> Option<LanguageVersion> {
    let release = Version::new(solc_version.major, solc_version.minor, solc_version.patch);
    let latest: Version = LanguageVersion::LATEST.into();
    if release > latest {
        return Some(LanguageVersion::LATEST);
    }

    LanguageVersion::try_from(release).ok()
}

/// Builds a Slang compilation unit over the file at `root_path`, resolving its
/// imports via `import_resolver` and reading them (and the root itself) from
/// disk.
///
/// Parse errors and unresolvable imports degrade gracefully: they surface as
/// diagnostics on the unit, and whatever still resolves is available. An
/// import that resolves to a path with no file behind it counts as
/// unresolvable. A missing root file yields an empty unit, so callers that
/// need to distinguish that case must check the root's existence themselves.
pub fn build_compilation_unit(
    root_path: &Path,
    language_version: LanguageVersion,
    import_resolver: &ImportResolver,
) -> CompilationUnit {
    let mut builder =
        CompilationBuilder::create(language_version, SourceProvider::new(import_resolver));
    builder.add_file(root_path.to_string_lossy().into_owned());

    builder.build()
}

#[cfg(test)]
mod tests {
    use slang_solidity_v2::diagnostics::{
        kinds::compilation::CompilationDiagnosticKind, DiagnosticKind,
    };

    use super::*;

    #[test]
    fn inheriting_from_a_missing_import_reports_it_as_unresolved() {
        let dir = tempfile::tempdir().expect("temp dir");
        let root = dir.path().join("Root.sol");
        std::fs::write(
            &root,
            "pragma solidity ^0.8.0;
import {Base} from \"./Missing.sol\";
contract Derived is Base {}
",
        )
        .expect("write root");

        let unit =
            build_compilation_unit(&root, LanguageVersion::LATEST, &ImportResolver::default());

        assert!(
            unit.diagnostics().iter().any(|diagnostic| matches!(
                diagnostic.kind(),
                DiagnosticKind::Compilation(CompilationDiagnosticKind::UnresolvedImport(_))
            )),
            "{:?}",
            unit.diagnostics()
        );
    }

    #[test]
    fn exact_supported_version() {
        assert_eq!(
            language_version_for_solc(&Version::new(0, 8, 24)),
            Some(LanguageVersion::V0_8_24)
        );
    }

    #[test]
    fn clamps_newer_versions_to_latest() {
        assert_eq!(
            language_version_for_solc(&Version::new(0, 9, 0)),
            Some(LanguageVersion::LATEST)
        );
    }

    #[test]
    fn versions_older_than_0_8_0_have_no_grammar() {
        assert_eq!(language_version_for_solc(&Version::new(0, 7, 6)), None);
    }

    #[test]
    fn ignores_build_and_prerelease_metadata() {
        let version =
            Version::parse("0.8.24-nightly.2024.1.1+commit.abcdef").expect("valid semver");

        assert_eq!(
            language_version_for_solc(&version),
            Some(LanguageVersion::V0_8_24)
        );
    }
}
