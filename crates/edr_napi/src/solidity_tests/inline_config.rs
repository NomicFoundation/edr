//! Surfaces ill-formed inline configuration (`forge-config:`/
//! `hardhat-config:` NatSpec directives) as a structured JS error.
//!
//! These also cover failures to locate, read or parse a test source, which
//! EIP-712 type collection shares, so a run using no inline configuration can
//! still fail with one.

use edr_solidity_tests::test_source_error as config_error;
use napi::{
    bindgen_prelude::{Either, Either5, Either6},
    Env, JsValue,
};
use napi_derive::napi;

/// A directive was missing the `=` separator.
#[napi(object)]
pub struct InlineConfigInvalidSyntax {
    /// Enum tag for JS.
    #[napi(ts_type = "\"InlineConfigInvalidSyntax\"")]
    pub kind: String,
    /// The offending directive line, stripped of comment decoration.
    pub directive: String,
}

/// A profile other than `default` was used.
#[napi(object)]
pub struct InlineConfigUnsupportedProfile {
    /// Enum tag for JS.
    #[napi(ts_type = "\"InlineConfigUnsupportedProfile\"")]
    pub kind: String,
    /// The unsupported profile name.
    pub profile: String,
}

/// An unknown configuration key was used.
#[napi(object)]
pub struct InlineConfigInvalidKey {
    /// Enum tag for JS.
    #[napi(ts_type = "\"InlineConfigInvalidKey\"")]
    pub kind: String,
    /// The offending key, exactly as written.
    pub key: String,
}

/// A key was used on a test of the wrong kind (e.g. `fuzz.*` on an invariant
/// test). Only function-level directives can produce this.
#[napi(object)]
pub struct InlineConfigInvalidKeyForTestType {
    /// Enum tag for JS.
    #[napi(ts_type = "\"InlineConfigInvalidKeyForTestType\"")]
    pub kind: String,
    /// The offending key, exactly as written.
    pub key: String,
    /// The kind of test the function is (`fuzz` or `invariant`).
    pub test_type: String,
}

/// A value did not match the expected type for its key.
#[napi(object)]
pub struct InlineConfigInvalidValue {
    /// Enum tag for JS.
    #[napi(ts_type = "\"InlineConfigInvalidValue\"")]
    pub kind: String,
    /// The offending key, exactly as written.
    pub key: String,
    /// The offending value, exactly as written.
    pub value: String,
    /// A description of the expected value type.
    pub expected: String,
}

/// The same key was specified more than once for the same function or
/// contract.
#[napi(object)]
pub struct InlineConfigDuplicateKey {
    /// Enum tag for JS.
    #[napi(ts_type = "\"InlineConfigDuplicateKey\"")]
    pub kind: String,
    /// The duplicated key, exactly as written.
    pub key: String,
}

/// The source's file could not be read at the path it was declared at.
#[napi(object)]
pub struct TestSourceFileNotFound {
    /// Enum tag for JS.
    #[napi(ts_type = "\"TestSourceFileNotFound\"")]
    pub kind: String,
    /// The path the source was expected at.
    pub path: String,
    /// Why reading it failed.
    pub reason: String,
}

/// A directive's offset could not be resolved to a line number within its
/// source, meaning the parsing stages disagree about the source text, so its
/// directives cannot be trusted.
#[napi(object)]
pub struct TestSourceDirectiveLocation {
    /// Enum tag for JS.
    #[napi(ts_type = "\"TestSourceDirectiveLocation\"")]
    pub kind: String,
    /// The contract the directive belongs to.
    pub contract: String,
    /// The test function the directive belongs to. Undefined when the directive
    /// is contract-level.
    pub function: Option<String>,
    /// Why resolving the location failed, including the directive problem
    /// that was being reported.
    pub reason: String,
}

/// The test source has no `testSourcePaths` entry, so it is not located, read,
/// or parsed.
#[napi(object)]
pub struct TestSourcePathNotProvided {
    /// Enum tag for JS.
    #[napi(ts_type = "\"TestSourcePathNotProvided\"")]
    pub kind: String,
}

/// The solc version the source was compiled with predates the oldest Solidity
/// grammar available, so the source cannot be parsed at all. Collecting inline
/// configuration and EIP-712 struct definitions requires solc 0.8.0 or newer,
/// and no source is exempt: a run that selects this one can only proceed with
/// collection disabled entirely, by omitting `testSourcePaths`.
#[napi(object)]
pub struct TestSourceUnsupportedSolcVersion {
    /// Enum tag for JS.
    #[napi(ts_type = "\"TestSourceUnsupportedSolcVersion\"")]
    pub kind: String,
    /// The solc version the source's artifact was compiled with.
    pub version: String,
}

/// The source does not parse, so nothing could be collected from it. A
/// partially-parsed source would silently miss struct definitions and
/// directives, so it is reported rather than half-collected.
#[napi(object)]
pub struct TestSourceParseErrors {
    /// Enum tag for JS.
    #[napi(ts_type = "\"TestSourceParseErrors\"")]
    pub kind: String,
    /// The syntax diagnostics, each located at its source line. Truncated to
    /// the first few, followed by a count of the rest.
    pub reasons: Vec<String>,
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

/// The problem in a single inline-config directive, as a discriminated union
/// over its `kind` tag — mirroring the Rust-side `TestSourceError` enum so
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

/// A source-level inline-config problem: one that could not be tied to a single
/// directive (e.g. an unreadable source, or one with no `testSourcePaths`
/// entry).
#[napi(object)]
pub struct TestSourceFileError {
    /// Discriminant tag for the `TestSourceError` union.
    #[napi(ts_type = "\"source\"")]
    pub kind: String,
    /// The solc source name the problem was found in (e.g.
    /// `project/test/Foo.t.sol`).
    pub source_name: String,
    /// The problem itself; discriminate on its `kind` tag.
    pub problem: TestSourceFileProblem,
}

/// A directive-level inline-config problem, located at the offending directive.
#[napi(object)]
pub struct InlineConfigDirectiveError {
    /// Discriminant tag for the `TestSourceError` union.
    #[napi(ts_type = "\"directive\"")]
    pub kind: String,
    /// The solc source name the problem was found in (e.g.
    /// `project/test/Foo.t.sol`).
    pub source_name: String,
    /// The contract the offending directive belongs to.
    pub contract: String,
    /// The test function the offending directive belongs to. Undefined when
    /// the directive is contract-level.
    pub function: Option<String>,
    /// The 1-based line of the offending directive within the source.
    pub line: u32,
    /// The problem itself; discriminate on its `kind` tag.
    pub problem: InlineConfigDirectiveProblem,
}

/// A single ill-formed inline-config entry, located so the user can find and
/// fix it. A discriminated union over `kind`: a `source`-level entry carries no
/// directive location, a `directive`-level entry carries the contract and line,
/// plus the function unless the directive is contract-level. Attached to the
/// rejected `runSolidityTests` promise as the `testSourceErrors` array on the
/// thrown error.
#[napi]
pub type TestSourceError = Either<TestSourceFileError, InlineConfigDirectiveError>;

fn to_source_problem(error: &config_error::TestSourceCollectError) -> TestSourceFileProblem {
    match error {
        config_error::TestSourceCollectError::RootFileNotFound { path, reason } => {
            Either5::A(TestSourceFileNotFound {
                kind: "TestSourceFileNotFound".to_owned(),
                path: path.clone(),
                reason: reason.clone(),
            })
        }
        config_error::TestSourceCollectError::DirectiveLocation {
            contract,
            function,
            reason,
        } => Either5::B(TestSourceDirectiveLocation {
            kind: "TestSourceDirectiveLocation".to_owned(),
            contract: contract.clone(),
            function: function.clone(),
            reason: reason.clone(),
        }),
        config_error::TestSourceCollectError::SourcePathNotProvided => {
            Either5::C(TestSourcePathNotProvided {
                kind: "TestSourcePathNotProvided".to_owned(),
            })
        }
        config_error::TestSourceCollectError::UnsupportedSolcVersion { version } => {
            Either5::D(TestSourceUnsupportedSolcVersion {
                kind: "TestSourceUnsupportedSolcVersion".to_owned(),
                version: version.to_string(),
            })
        }
        config_error::TestSourceCollectError::SourceParseErrors { reasons } => {
            Either5::E(TestSourceParseErrors {
                kind: "TestSourceParseErrors".to_owned(),
                reasons: reasons.clone(),
            })
        }
    }
}

fn to_directive_problem(error: &config_error::InlineConfigError) -> InlineConfigDirectiveProblem {
    match error {
        config_error::InlineConfigError::InvalidSyntax { line } => {
            Either6::A(InlineConfigInvalidSyntax {
                kind: "InlineConfigInvalidSyntax".to_owned(),
                directive: line.clone(),
            })
        }
        config_error::InlineConfigError::UnsupportedProfile { profile } => {
            Either6::B(InlineConfigUnsupportedProfile {
                kind: "InlineConfigUnsupportedProfile".to_owned(),
                profile: profile.clone(),
            })
        }
        config_error::InlineConfigError::InvalidKey { key } => Either6::C(InlineConfigInvalidKey {
            kind: "InlineConfigInvalidKey".to_owned(),
            key: key.clone(),
        }),
        config_error::InlineConfigError::InvalidKeyForTestType { key, test_type } => {
            Either6::D(InlineConfigInvalidKeyForTestType {
                kind: "InlineConfigInvalidKeyForTestType".to_owned(),
                key: key.clone(),
                test_type: test_type.clone(),
            })
        }
        config_error::InlineConfigError::InvalidValue {
            key,
            value,
            expected,
        } => Either6::E(InlineConfigInvalidValue {
            kind: "InlineConfigInvalidValue".to_owned(),
            key: key.clone(),
            value: value.clone(),
            expected: (*expected).to_owned(),
        }),
        config_error::InlineConfigError::DuplicateKey { key } => {
            Either6::F(InlineConfigDuplicateKey {
                kind: "InlineConfigDuplicateKey".to_owned(),
                key: key.clone(),
            })
        }
    }
}

fn to_entry(item: &config_error::TestSourceErrorItem) -> TestSourceError {
    let source_name = item.source_name.to_string_lossy().into_owned();
    match &item.problem {
        config_error::TestSourceProblem::Source(error) => Either::A(TestSourceFileError {
            kind: "source".to_owned(),
            source_name,
            problem: to_source_problem(error),
        }),
        config_error::TestSourceProblem::Directive(
            config_error::InlineConfigDirectiveError {
                contract,
                function,
                line,
                error,
            },
        ) => Either::B(InlineConfigDirectiveError {
            kind: "directive".to_owned(),
            source_name,
            contract: contract.clone(),
            function: function.clone(),
            line: *line,
            problem: to_directive_problem(error),
        }),
    }
}

/// Builds the error that rejects `runSolidityTests` when inline-config
/// validation fails, carrying the structured, located problems on the JS error
/// as its `testSourceErrors` property.
///
/// Must be called on the JS thread (it builds JS values); the reject path in
/// [`crate::context`] does so from the deferred's resolver. Falls back to a
/// plain message-only error if building the structured object fails.
pub(crate) fn to_napi_error(env: &Env, errors: &config_error::TestSourceErrors) -> napi::Error {
    build_structured_error(env, errors)
        .unwrap_or_else(|_| napi::Error::from_reason(summary(errors)))
}

fn build_structured_error(
    env: &Env,
    errors: &config_error::TestSourceErrors,
) -> napi::Result<napi::Error> {
    let mut error_object = env.create_error(napi::Error::from_reason(summary(errors)))?;
    let items: Vec<TestSourceError> = errors.items().iter().map(to_entry).collect();
    error_object.set("testSourceErrors", items)?;
    Ok(napi::Error::from(error_object.to_unknown()))
}

fn summary(errors: &config_error::TestSourceErrors) -> String {
    format!("Could not collect from the test sources:\n{errors}")
}
