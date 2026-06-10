//! The error surfaced when an inline-config directive is malformed.

/// Errors produced while parsing or validating inline configuration.
#[derive(Clone, Debug, thiserror::Error, PartialEq, Eq)]
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
}
