//! Problems found while locating, reading or parsing a test source.
//!
//! These are not about inline configuration, though ill-formed directives are
//! reported alongside them: a run using no directives at all can still be
//! rejected by one of these.

use napi::bindgen_prelude::Either5;
use napi_derive::napi;

use crate::structured_error::impl_structured_napi_error;

impl_structured_napi_error! {
    /// The source's file could not be read at the path it was declared at.
    pub struct TestSourceFileNotFound {
        /// The path the source was expected at.
        pub path: String,
        /// Why reading it failed.
        pub reason: String,
    }
}

impl_structured_napi_error! {
    /// A directive's offset could not be resolved to a line number within its
    /// source, meaning the parsing stages disagree about the source text, so
    /// its directives cannot be trusted.
    pub struct TestSourceDirectiveLocation {
        /// The contract the directive belongs to.
        pub contract: String,
        /// The test function the directive belongs to. Undefined when the
        /// directive is contract-level.
        pub function: Option<String>,
        /// Why resolving the location failed, including the directive problem
        /// that was being reported.
        pub reason: String,
    }
}

impl_structured_napi_error! {
    /// The test source has no `testSourcePaths` entry, so it is not located,
    /// read, or parsed.
    pub struct TestSourcePathNotProvided {}
}

impl_structured_napi_error! {
    /// The solc version the source was compiled with predates the oldest
    /// Solidity grammar available, so the source cannot be parsed at all.
    /// Collecting inline configuration and EIP-712 struct definitions requires
    /// solc 0.8.0 or newer, and no source is exempt: a run that selects this
    /// one can only proceed with collection disabled entirely, by omitting
    /// `testSourcePaths`.
    pub struct TestSourceUnsupportedSolcVersion {
        /// The solc version the source's artifact was compiled with.
        pub version: String,
    }
}

impl_structured_napi_error! {
    /// The source does not parse, so nothing could be collected from it. A
    /// partially-parsed source would silently miss struct definitions and
    /// directives, so it is reported rather than half-collected.
    pub struct TestSourceParseErrors {
        /// The syntax diagnostics, each located at its source line. Truncated
        /// to the first few, followed by a count of the rest.
        pub reasons: Vec<String>,
    }
}

/// A source-level problem, as a discriminated union over its `kind` tag. These
/// cannot be pinned to a single directive line, so they carry no line.
#[napi]
pub type TestSourceFileProblem = Either5<
    TestSourceFileNotFound,
    TestSourceDirectiveLocation,
    TestSourcePathNotProvided,
    TestSourceUnsupportedSolcVersion,
    TestSourceParseErrors,
>;

impl_structured_napi_error! {
    /// A problem with the source itself, which no single directive can be
    /// blamed for.
    ///
    /// Its tag names the half of the `TestSourceError` union it belongs to,
    /// not its own type, because the two halves differ in shape rather than in
    /// what went wrong.
    pub struct TestSourceFileError tagged "source" {
        /// The solc source name the problem was found in (e.g.
        /// `project/test/Foo.t.sol`).
        pub source_name: String,
        /// The problem itself; discriminate on its `kind` tag.
        pub problem: TestSourceFileProblem,
    }
}
