//! The errors surfaced while resolving inline configuration.

use slang_solidity_v2::utils::FromSemverError;

/// Errors produced while collecting a source's inline configuration before the
/// individual directives are parsed (see [`InlineConfigError`]).
#[derive(Debug, thiserror::Error, PartialEq)]
pub enum InlineConfigCollectError {
    /// The source's solc version has no supported Slang grammar.
    #[error(transparent)]
    InvalidSolcVersion(#[from] FromSemverError),
    /// A test source's file was not found at the path it was declared at.
    #[error("could not read inline-config source '{path}': {reason}")]
    RootFileNotFound {
        /// The path the source was expected at.
        path: String,
        /// Why reading it failed.
        reason: String,
    },
}

// TODO: `derive(Clone)` once `FromSemverError` implements `Clone`.
impl Clone for InlineConfigCollectError {
    fn clone(&self) -> Self {
        match self {
            Self::InvalidSolcVersion(FromSemverError::UnexpectedMetadata) => {
                Self::InvalidSolcVersion(FromSemverError::UnexpectedMetadata)
            }
            Self::InvalidSolcVersion(FromSemverError::UnsupportedVersion) => {
                Self::InvalidSolcVersion(FromSemverError::UnsupportedVersion)
            }
            Self::RootFileNotFound { path, reason } => Self::RootFileNotFound {
                path: path.clone(),
                reason: reason.clone(),
            },
        }
    }
}

/// Errors produced while parsing or validating inline configuration.
#[derive(Clone, Debug, thiserror::Error, PartialEq)]
pub enum InlineConfigError {
    /// A directive was missing the `=` separator.
    #[error("Invalid inline config syntax in {test_function}: missing '=' in `{line}`")]
    InvalidSyntax {
        /// The function the directive belongs to.
        test_function: String,
        /// The offending directive line.
        line: String,
    },
    /// A profile other than `default` was used.
    #[error(
        "Unsupported inline config profile `{profile}` in {test_function}; only `default` is supported"
    )]
    UnsupportedProfile {
        /// The function the directive belongs to.
        test_function: String,
        /// The unsupported profile name.
        profile: String,
    },
    /// An unknown configuration key was used.
    #[error("Invalid inline config key `{key}` in {test_function}")]
    InvalidKey {
        /// The function the directive belongs to.
        test_function: String,
        /// The offending (raw) key.
        key: String,
    },
    /// A key was used on a test of the wrong kind (e.g. `fuzz.*` on an
    /// invariant test).
    #[error("Inline config key `{key}` is not valid for {test_type} test {test_function}")]
    InvalidKeyForTestType {
        /// The function the directive belongs to.
        test_function: String,
        /// The offending (raw) key.
        key: String,
        /// The kind of test (`fuzz` or `invariant`).
        test_type: String,
    },
    /// A value did not match the expected type for its key.
    #[error(
        "Invalid value `{value}` for inline config key `{key}` in {test_function}: expected {expected}"
    )]
    InvalidValue {
        /// The function the directive belongs to.
        test_function: String,
        /// The offending (raw) key.
        key: String,
        /// The offending value.
        value: String,
        /// A description of the expected value type.
        expected: &'static str,
    },
    /// The same key was specified more than once for a function.
    #[error("Duplicate inline config key `{key}` in {test_function}")]
    DuplicateKey {
        /// The function the directive belongs to.
        test_function: String,
        /// The duplicated (raw) key.
        key: String,
    },
    /// Collecting the source's inline configuration failed before its
    /// directives could be parsed.
    #[error(transparent)]
    Collect(#[from] InlineConfigCollectError),
}
