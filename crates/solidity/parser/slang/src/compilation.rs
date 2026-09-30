//! Building Slang compilation units over on-disk Solidity sources.

use std::path::{Path, PathBuf};

use semver::Version;
use slang_solidity_v2::{
    compilation::{CompilationBuilder, CompilationUnit},
    utils::LanguageVersion,
};

use crate::resolver::{ImportResolver, RootSource, SourceProvider};

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

/// A compilation unit built over a root file, together with the root's text.
pub struct RootCompilation {
    /// The unit over the root and whatever of its import closure resolved.
    pub unit: CompilationUnit,
    /// The root's source text, read once and shared with Slang's parse. The
    /// unit does not retain comments, so anything recovered from them, such
    /// as NatSpec directives, has to come from here.
    pub root_content: String,
}

/// The root file could not be read, so there is nothing to build a unit over.
#[derive(Debug, thiserror::Error)]
#[error("could not read `{path}`: {source}")]
pub struct ReadRootError {
    /// The path the root was expected at.
    pub path: PathBuf,
    /// Why reading it failed.
    pub source: std::io::Error,
}

/// Builds a Slang compilation unit over the file at `root_path`, resolving its
/// imports via `import_resolver` and reading them from disk.
///
/// Parse errors and unresolvable imports degrade gracefully: they surface as
/// diagnostics on the unit, and whatever still resolves is available. An
/// import that resolves to a path with no file behind it counts as
/// unresolvable. Only the root has to be readable, because there is no unit
/// without it.
pub fn build_compilation_unit(
    root_path: &Path,
    language_version: LanguageVersion,
    import_resolver: &ImportResolver,
) -> Result<RootCompilation, ReadRootError> {
    let root_content = std::fs::read_to_string(root_path).map_err(|source| ReadRootError {
        path: root_path.to_path_buf(),
        source,
    })?;
    let root_id = root_path.to_string_lossy().into_owned();

    let provider = SourceProvider::new(
        import_resolver,
        RootSource {
            id: &root_id,
            content: &root_content,
        },
    );
    let mut builder = CompilationBuilder::create(language_version, provider);
    builder.add_file(root_id.clone());

    Ok(RootCompilation {
        unit: builder.build(),
        root_content,
    })
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
            build_compilation_unit(&root, LanguageVersion::LATEST, &ImportResolver::default())
                .expect("the root exists")
                .unit;

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
    fn returns_the_root_text_it_parsed() {
        let dir = tempfile::tempdir().expect("temp dir");
        let root = dir.path().join("Root.sol");
        let source =
            "pragma solidity ^0.8.0;\n/// forge-config: default.fuzz.runs = 1\ncontract C {}\n";
        std::fs::write(&root, source).expect("write root");

        let compilation =
            build_compilation_unit(&root, LanguageVersion::LATEST, &ImportResolver::default())
                .expect("the root exists");

        assert_eq!(compilation.root_content, source);
        assert!(compilation.unit.file(&root.to_string_lossy()).is_some());
    }

    #[test]
    fn missing_root_is_an_error() {
        let dir = tempfile::tempdir().expect("temp dir");
        let root = dir.path().join("Missing.sol");

        let Err(error) =
            build_compilation_unit(&root, LanguageVersion::LATEST, &ImportResolver::default())
        else {
            panic!("the root does not exist");
        };

        assert_eq!(error.path, root);
        assert_eq!(error.source.kind(), std::io::ErrorKind::NotFound);
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
