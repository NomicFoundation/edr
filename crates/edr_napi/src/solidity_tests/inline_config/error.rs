//! Problems in a single inline-configuration directive
//! (`forge-config:`/`hardhat-config:` NatSpec).
//!
//! Unlike the source-level problems in [`crate::solidity_tests::parsing`],
//! every one of these is about a directive a test author wrote, and is located
//! at the line that carries it.

use napi::bindgen_prelude::Either6;
use napi_derive::napi;

use crate::structured_error::impl_structured_napi_error;

impl_structured_napi_error! {
    /// A directive was missing the `=` separator.
    pub struct InlineConfigInvalidSyntax {
        /// The offending directive line, stripped of comment decoration.
        pub directive: String,
    }
}

impl_structured_napi_error! {
    /// A profile other than `default` was used.
    pub struct InlineConfigUnsupportedProfile {
        /// The unsupported profile name.
        pub profile: String,
    }
}

impl_structured_napi_error! {
    /// An unknown configuration key was used.
    pub struct InlineConfigInvalidKey {
        /// The offending key, exactly as written.
        pub key: String,
    }
}

impl_structured_napi_error! {
    /// A key was used on a test of the wrong kind (e.g. `fuzz.*` on an
    /// invariant test).
    pub struct InlineConfigInvalidKeyForTestType {
        /// The offending key, exactly as written.
        pub key: String,
        /// The kind of test the function is (`fuzz` or `invariant`).
        pub test_type: String,
    }
}

impl_structured_napi_error! {
    /// A value did not match the expected type for its key.
    pub struct InlineConfigInvalidValue {
        /// The offending key, exactly as written.
        pub key: String,
        /// The offending value, exactly as written.
        pub value: String,
        /// What the key expects instead.
        pub expected: String,
    }
}

impl_structured_napi_error! {
    /// The same key was set twice for the same target.
    pub struct InlineConfigDuplicateKey {
        /// The duplicated key.
        pub key: String,
    }
}

/// The problem in a single inline-config directive, as a discriminated union
/// over its `kind` tag — mirroring the Rust-side `InlineConfigError` enum so
/// consumers can map each problem onto their own error types.
#[napi]
pub type InlineConfigDirectiveProblem = Either6<
    InlineConfigInvalidSyntax,
    InlineConfigUnsupportedProfile,
    InlineConfigInvalidKey,
    InlineConfigInvalidKeyForTestType,
    InlineConfigInvalidValue,
    InlineConfigDuplicateKey,
>;

impl_structured_napi_error! {
    /// A directive-level problem, located at the offending directive.
    ///
    /// Its tag names the half of the `TestSourceError` union it belongs to,
    /// not its own type, because the two halves differ in shape rather than in
    /// what went wrong.
    pub struct InlineConfigDirectiveError tagged "directive" {
        /// The solc source name the problem was found in (e.g.
        /// `project/test/Foo.t.sol`).
        pub source_name: String,
        /// The contract the offending directive belongs to.
        pub contract: String,
        /// The test function the offending directive belongs to. Undefined
        /// when the directive is contract-level.
        pub function: Option<String>,
        /// The 1-based line of the offending directive within the source.
        pub line: u32,
        /// The problem itself; discriminate on its `kind` tag.
        pub problem: InlineConfigDirectiveProblem,
    }
}
