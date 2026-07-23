//! The errors surfaced while resolving inline configuration.

use std::path::PathBuf;

use edr_solidity_parser_slang::UnsupportedSolcVersionError;

/// Errors produced while collecting a source's inline configuration before the
/// individual directives are parsed (see [`InlineConfigError`]).
#[derive(Clone, Debug, thiserror::Error, PartialEq)]
pub enum InlineConfigCollectError {
    /// The source's solc version has no supported Slang grammar.
    #[error(transparent)]
    InvalidSolcVersion(#[from] UnsupportedSolcVersionError),
    /// A test source's file was not found at the path it was declared at.
    #[error("could not read inline-config source '{path}': {reason}")]
    RootFileNotFound {
        /// The path the source was expected at.
        path: String,
        /// Why reading it failed.
        reason: String,
    },
    /// The test source has no `test_source_paths` entry, so it cannot be
    /// located, read, and parsed.
    #[error("no source path was provided for the test source")]
    SourcePathNotProvided,
    /// The test source's content could not be parsed to an AST.
    #[error("the test source could not be parsed: {reason}")]
    ParseError {
        /// The parse error, located at its source line.
        reason: String,
    },
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
#[derive(Clone, Debug, thiserror::Error, PartialEq)]
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

/// A single inline-config problem together with enough location to point the
/// user at it — modeled on the stack-trace `SourceReference` surfaced to
/// consumers.
#[derive(Clone, Debug, PartialEq)]
pub struct InlineConfigErrorItem {
    /// The solc source name the problem was found in (e.g.
    /// `project/test/Foo.t.sol`).
    pub source: PathBuf,
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
#[derive(Clone, Debug, PartialEq)]
pub enum InlineConfigProblem {
    /// A problem found while collecting the source, before its directives could
    /// be parsed. Kept structured so consumers can map it onto their own error
    /// types; render it with `to_string()` for a human.
    Source(InlineConfigCollectError),
    /// A problem in a specific directive. Kept structured so consumers can map
    /// it onto their own error types; render it with `to_string()` for a human.
    Directive {
        /// The contract the offending directive belongs to.
        contract: String,
        /// The test function the offending directive belongs to, or `None` for
        /// a contract-level directive.
        function: Option<String>,
        /// The 1-based line of the offending directive within the source.
        line: u32,
        /// The problem itself.
        error: InlineConfigError,
    },
}

impl std::fmt::Display for InlineConfigErrorItem {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.source.display())?;
        match &self.problem {
            InlineConfigProblem::Source(error) => write!(f, ": {error}"),
            InlineConfigProblem::Directive {
                contract,
                function,
                line,
                error,
            } => match function {
                Some(function) => write!(f, ":{line}: {contract}.{function}: {error}"),
                None => write!(f, ":{line}: {contract}: {error}"),
            },
        }
    }
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
    pub(crate) fn new(items: Vec<InlineConfigErrorItem>) -> Self {
        Self { items }
    }

    /// The individual problems, each with its location, for structured
    /// reporting to consumers.
    pub fn items(&self) -> &[InlineConfigErrorItem] {
        &self.items
    }
}

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

impl std::error::Error for InlineConfigErrors {}
