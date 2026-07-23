//! Building Slang compilation units over on-disk Solidity sources.

use std::path::Path;

use semver::Version;
use slang_solidity_v2::{
    compilation::{CompilationBuilder, CompilationUnit},
    utils::{FromSemverError, LanguageVersion},
};

use crate::resolver::{ImportResolver, SourceProvider};

/// The solc version a source was compiled with maps to no supported Slang
/// grammar.
#[derive(Debug, thiserror::Error, PartialEq)]
#[error("solc version {version} is not supported by Slang: {source}")]
pub struct UnsupportedSolcVersionError {
    /// The rejected solc version.
    pub version: Version,
    /// The underlying version-mapping error.
    #[source]
    pub source: FromSemverError,
}

// TODO: `derive(Clone)` once `FromSemverError` implements `Clone`.
impl Clone for UnsupportedSolcVersionError {
    fn clone(&self) -> Self {
        // Not a needless match: `FromSemverError` is neither `Clone` nor
        // `Copy`, so the identity match is the only way to duplicate it out
        // of the borrow.
        #[allow(clippy::needless_match)]
        let source = match self.source {
            FromSemverError::UnexpectedMetadata => FromSemverError::UnexpectedMetadata,
            FromSemverError::UnsupportedVersion => FromSemverError::UnsupportedVersion,
        };
        Self {
            version: self.version.clone(),
            source,
        }
    }
}

/// Builds a Slang compilation unit over the file at `root_path`, resolving its
/// imports via `import_resolver` and reading them (and the root itself) from
/// disk.
///
/// Parse errors and unresolvable imports degrade gracefully: they surface as
/// diagnostics on the unit, and whatever still resolves is available. A
/// missing root file yields an empty unit — callers that need to distinguish
/// that case must check the root's existence themselves. Fails only if
/// `solc_version` maps to no supported Slang grammar.
pub fn build_compilation_unit(
    root_path: &Path,
    solc_version: Version,
    import_resolver: &ImportResolver,
) -> Result<CompilationUnit, UnsupportedSolcVersionError> {
    let language_version = to_language_version(solc_version.clone()).map_err(|source| {
        UnsupportedSolcVersionError {
            version: solc_version,
            source,
        }
    })?;

    let mut builder =
        CompilationBuilder::create(language_version, SourceProvider::new(import_resolver));
    builder.add_file(root_path.to_string_lossy().into_owned());

    Ok(builder.build())
}

/// Maps a solc [`Version`] to a Slang [`LanguageVersion`]; clamping versions
/// newer than Slang supports down to its latest grammar.
fn to_language_version(solc_version: Version) -> Result<LanguageVersion, FromSemverError> {
    // Fall back to the latest Slang grammar for any solc version newer than what
    // Slang supports.
    let latest: Version = LanguageVersion::LATEST.into();
    if solc_version > latest {
        Ok(LanguageVersion::LATEST)
    } else {
        LanguageVersion::try_from(solc_version)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    mod version_mapping {
        use super::*;

        #[test]
        fn exact_supported_version() {
            assert_eq!(
                to_language_version(Version::new(0, 8, 24)).unwrap(),
                LanguageVersion::V0_8_24
            );
        }

        #[test]
        fn clamps_newer_versions_to_latest() {
            assert_eq!(
                to_language_version(Version::new(0, 9, 0)).unwrap(),
                LanguageVersion::LATEST
            );
        }

        #[test]
        fn rejects_versions_older_than_0_8_0() {
            assert!(matches!(
                to_language_version(Version::new(0, 7, 6)),
                Err(FromSemverError::UnsupportedVersion)
            ));
        }

        #[test]
        fn rejects_versions_with_build_and_prerelease_metadata() {
            let version = Version::parse("0.8.24+commit.abcdef").unwrap();
            assert!(matches!(
                to_language_version(version),
                Err(FromSemverError::UnexpectedMetadata)
            ));
        }
    }

    #[test]
    fn unsupported_version_error_carries_the_version() {
        match build_compilation_unit(
            Path::new("/does/not/matter.sol"),
            Version::new(0, 7, 6),
            &ImportResolver::default(),
        ) {
            Err(error) => assert_eq!(error.version, Version::new(0, 7, 6)),
            Ok(_) => panic!("0.7.6 has no Slang grammar"),
        }
    }
}
