//! Surfaces ill-formed inline configuration (`forge-config:`/
//! `hardhat-config:` NatSpec directives) as a structured JS error.

use edr_solidity_tests::inline_config::error as config_error;
use napi::{
    bindgen_prelude::{Either, Either3, Either6},
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
pub struct InlineConfigSourceFileNotFound {
    /// Enum tag for JS.
    #[napi(ts_type = "\"InlineConfigSourceFileNotFound\"")]
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
pub struct InlineConfigDirectiveLocation {
    /// Enum tag for JS.
    #[napi(ts_type = "\"InlineConfigDirectiveLocation\"")]
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
pub struct InlineConfigSourcePathNotProvided {
    /// Enum tag for JS.
    #[napi(ts_type = "\"InlineConfigSourcePathNotProvided\"")]
    pub kind: String,
}

/// A source-level problem, as a discriminated union over its `kind` tag. These
/// cannot be pinned to a single directive line, so they carry no line.
#[napi]
pub type InlineConfigSourceProblem = Either3<
    InlineConfigSourceFileNotFound,
    InlineConfigDirectiveLocation,
    InlineConfigSourcePathNotProvided,
>;

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

/// A source-level inline-config problem: one that could not be tied to a single
/// directive (e.g. an unsupported solc version or an unreadable source).
#[napi(object)]
pub struct InlineConfigSourceError {
    /// Discriminant tag for the `InlineConfigError` union.
    #[napi(ts_type = "\"source\"")]
    pub kind: String,
    /// The solc source name the problem was found in (e.g.
    /// `project/test/Foo.t.sol`).
    pub source_name: String,
    /// The problem itself; discriminate on its `kind` tag.
    pub problem: InlineConfigSourceProblem,
}

/// A directive-level inline-config problem, located at the offending directive.
#[napi(object)]
pub struct InlineConfigDirectiveError {
    /// Discriminant tag for the `InlineConfigError` union.
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
/// rejected `runSolidityTests` promise as the `inlineConfigErrors` array on the
/// thrown error.
#[napi]
pub type InlineConfigError = Either<InlineConfigSourceError, InlineConfigDirectiveError>;

fn to_source_problem(error: &config_error::InlineConfigCollectError) -> InlineConfigSourceProblem {
    match error {
        config_error::InlineConfigCollectError::RootFileNotFound { path, reason } => {
            Either3::A(InlineConfigSourceFileNotFound {
                kind: "InlineConfigSourceFileNotFound".to_owned(),
                path: path.clone(),
                reason: reason.clone(),
            })
        }
        config_error::InlineConfigCollectError::DirectiveLocation {
            contract,
            function,
            reason,
        } => Either3::B(InlineConfigDirectiveLocation {
            kind: "InlineConfigDirectiveLocation".to_owned(),
            contract: contract.clone(),
            function: function.clone(),
            reason: reason.clone(),
        }),
        config_error::InlineConfigCollectError::SourcePathNotProvided => {
            Either3::C(InlineConfigSourcePathNotProvided {
                kind: "InlineConfigSourcePathNotProvided".to_owned(),
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

fn to_entry(item: &config_error::InlineConfigErrorItem) -> InlineConfigError {
    let source_name = item.source_name.to_string_lossy().into_owned();
    match &item.problem {
        config_error::InlineConfigProblem::Source(error) => Either::A(InlineConfigSourceError {
            kind: "source".to_owned(),
            source_name,
            problem: to_source_problem(error),
        }),
        config_error::InlineConfigProblem::Directive(
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
/// as its `inlineConfigErrors` property.
///
/// Must be called on the JS thread (it builds JS values); the reject path in
/// [`crate::context`] does so from the deferred's resolver. Falls back to a
/// plain message-only error if building the structured object fails.
pub(crate) fn to_napi_error(env: &Env, errors: &config_error::InlineConfigErrors) -> napi::Error {
    build_structured_error(env, errors)
        .unwrap_or_else(|_| napi::Error::from_reason(summary(errors)))
}

fn build_structured_error(
    env: &Env,
    errors: &config_error::InlineConfigErrors,
) -> napi::Result<napi::Error> {
    let mut error_object = env.create_error(napi::Error::from_reason(summary(errors)))?;
    let items: Vec<InlineConfigError> = errors.items().iter().map(to_entry).collect();
    error_object.set("inlineConfigErrors", items)?;
    Ok(napi::Error::from(error_object.to_unknown()))
}

fn summary(errors: &config_error::InlineConfigErrors) -> String {
    format!("Found invalid inline configuration in test sources:\n{errors}")
}
