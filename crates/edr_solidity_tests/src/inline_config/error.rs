//! The errors surfaced while resolving inline configuration.
//!
//! These also cover failures to locate or read a test source, which EIP-712
//! type collection shares, so a run using no inline configuration can still
//! fail with one.

use std::path::PathBuf;

/// Errors produced while collecting a source's inline configuration before the
/// individual directives are parsed (see [`InlineConfigError`]).
#[derive(Clone, Debug, thiserror::Error, PartialEq)]
pub enum InlineConfigCollectError {
    /// A test source's file was not found at the path it was declared at.
    #[error("could not read inline-config source '{path}': {reason}")]
    RootFileNotFound {
        /// The path the source was expected at.
        path: String,
        /// Why reading it failed.
        reason: String,
    },
    /// The test source has no `test_source_paths` entry, so it is not located,
    /// read, or parsed.
    #[error("no source path was provided for the test source")]
    SourcePathNotProvided,
    /// A directive's offset could not be resolved to a line number: it lies
    /// outside the source text (or the line count overflows), meaning the
    /// parsing stages disagree about the source, so its directives cannot be
    /// trusted.
    #[error(
        "could not locate a directive of `{contract}{}{}`: {reason}",
        if function.is_some() { "." } else { "" },
        function.as_deref().unwrap_or_default()
    )]
    DirectiveLocation {
        /// The contract the directive belongs to.
        contract: String,
        /// The test function the directive belongs to, or `None` for a
        /// contract-level directive.
        function: Option<String>,
        /// Why resolving the location failed, including the directive problem
        /// that was being reported.
        reason: String,
    },
}

/// Errors produced while parsing or validating inline configuration.
#[derive(Clone, Debug, thiserror::Error, PartialEq, Eq)]
pub enum InlineConfigError {
    /// A directive was missing the `=` separator.
    #[error("missing '=' in `{line}`")]
    InvalidSyntax {
        /// The offending directive line.
        line: String,
    },
    /// A profile other than `default` was used.
    #[error("unsupported profile `{profile}`; only `default` is supported")]
    UnsupportedProfile {
        /// The unsupported profile name.
        profile: String,
    },
    /// An unknown configuration key was used.
    #[error("invalid key `{key}`")]
    InvalidKey {
        /// The offending (raw) key.
        key: String,
    },
    /// A key was used on a test of the wrong kind (e.g. `fuzz.*` on an
    /// invariant test). Only function-level directives can produce this.
    #[error("key `{key}` is not valid for {test_type} tests")]
    InvalidKeyForTestType {
        /// The offending (raw) key.
        key: String,
        /// The kind of test the function is (`fuzz` or `invariant`).
        test_type: String,
    },
    /// A value did not match the expected type for its key.
    #[error("invalid value `{value}` for key `{key}`: expected {expected}")]
    InvalidValue {
        /// The offending (raw) key.
        key: String,
        /// The offending value.
        value: String,
        /// A description of the expected value type.
        expected: &'static str,
    },
    /// The same key was specified more than once for the same function or
    /// contract.
    #[error("duplicate key `{key}`")]
    DuplicateKey {
        /// The duplicated (raw) key.
        key: String,
    },
}

/// A problem in a directive, located at the line it was written on.
#[derive(Clone, Debug, thiserror::Error, PartialEq, Eq)]
#[error(
    ":{line}: {contract}{}{}: {error}",
    if function.is_some() { "." } else { "" },
    function.as_deref().unwrap_or_default()
)]
pub struct InlineConfigDirectiveError {
    /// The contract the offending directive belongs to.
    pub contract: String,
    /// The test function the offending directive belongs to, or `None` for a
    /// contract-level directive.
    pub function: Option<String>,
    /// The 1-based line of the offending directive within the source.
    pub line: u32,
    /// The problem itself.
    pub error: InlineConfigError,
}

/// A single inline-config problem together with enough location to point the
/// user at it — modeled on the stack-trace `SourceReference` surfaced to
/// consumers.
#[derive(Clone, Debug, thiserror::Error, PartialEq)]
#[error("{}: {problem}", source_name.display())]
pub struct InlineConfigErrorItem {
    /// The solc source name the problem was found in (e.g.
    /// `project/test/Foo.t.sol`).
    pub source_name: PathBuf,
    /// The problem, together with whatever location detail applies to it.
    pub problem: InlineConfigProblem,
}

/// An inline-config problem, split by whether it can be pinned to a single
/// directive line.
///
/// A source-level problem (e.g. an unsupported solc version, an unreadable
/// source file, or a directive whose location could not be resolved) carries
/// no contract/function/line — there is no directive line to point at. A
/// directive-level problem always carries the contract and line; the function
/// is absent for a contract-level directive.
#[derive(Clone, Debug, thiserror::Error, PartialEq)]
pub enum InlineConfigProblem {
    /// A problem found while collecting the source, before its directives could
    /// be parsed. Kept structured so consumers can map it onto their own error
    /// types; render it with `to_string()` for a human.
    #[error(transparent)]
    Source(#[from] InlineConfigCollectError),
    /// A problem in a specific directive. Kept structured so consumers can map
    /// it onto their own error types; render it with `to_string()` for a human.
    #[error(transparent)]
    Directive(#[from] InlineConfigDirectiveError),
}

/// Every inline-config problem found while collecting the test sources.
///
/// When collection surfaces any problem, runner creation fails and the whole
/// test run is aborted before any suite executes. At most one problem is
/// reported per directive target — each test function, plus each contract's
/// own directives — across every source.
#[derive(Clone, Debug, PartialEq)]
pub struct InlineConfigErrors {
    items: Vec<InlineConfigErrorItem>,
}

impl InlineConfigErrors {
    /// The individual problems, each with its location, for structured
    /// reporting to consumers.
    pub fn items(&self) -> &[InlineConfigErrorItem] {
        &self.items
    }
}

impl std::error::Error for InlineConfigErrors {}

impl std::fmt::Display for InlineConfigErrors {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for (index, item) in self.items.iter().enumerate() {
            if index > 0 {
                writeln!(f)?;
            }
            write!(f, "  {item}")?;
        }
        Ok(())
    }
}

impl TryFrom<Vec<InlineConfigErrorItem>> for InlineConfigErrors {
    type Error = NoInlineConfigProblems;

    /// Fails on an empty vector: an `InlineConfigErrors` carrying no problem
    /// would render as an empty report and abort a run for no stated reason.
    fn try_from(items: Vec<InlineConfigErrorItem>) -> Result<Self, Self::Error> {
        if items.is_empty() {
            Err(NoInlineConfigProblems)
        } else {
            Ok(Self { items })
        }
    }
}

/// Returned when [`InlineConfigErrors`] is built from an empty set of
/// problems, which would describe a failure that did not happen.
#[derive(Clone, Copy, Debug, thiserror::Error)]
#[error("no inline-config problems to report")]
pub struct NoInlineConfigProblems;
