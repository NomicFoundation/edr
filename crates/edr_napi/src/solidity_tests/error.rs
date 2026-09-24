//! Turning collected test-source problems into one structured JS error.
//!
//! The problems span two halves — the source itself, and the directives within
//! it — so the union that joins them lives here rather than in either half.

pub mod inline_config;
pub mod parsing;

use edr_solidity_tests::test_source_error as collect_error;
use napi::{
    bindgen_prelude::{Either, Either5, Either6},
    Env, JsValue,
};
use napi_derive::napi;

use self::{
    inline_config::{
        InlineConfigDirectiveError, InlineConfigDirectiveProblem, InlineConfigDuplicateKey,
        InlineConfigInvalidKey, InlineConfigInvalidKeyForTestType, InlineConfigInvalidSyntax,
        InlineConfigInvalidValue, InlineConfigUnsupportedProfile,
    },
    parsing::{
        TestSourceDirectiveLocation, TestSourceFileError, TestSourceFileNotFound,
        TestSourceFileProblem, TestSourceParseErrors, TestSourcePathNotProvided,
        TestSourceUnsupportedSolcVersion,
    },
};

/// A single problem found in the test sources, located so the user can find and
/// fix it. A discriminated union over `kind`: a `source`-level entry carries no
/// directive location, a `directive`-level entry carries the contract and line,
/// plus the function unless the directive is contract-level. Attached to the
/// rejected `runSolidityTests` promise as the `testSourceErrors` array on the
/// thrown error.
#[napi]
pub type TestSourceError = Either<TestSourceFileError, InlineConfigDirectiveError>;

fn to_source_problem(error: collect_error::TestSourceCollectError) -> TestSourceFileProblem {
    match error {
        collect_error::TestSourceCollectError::RootFileNotFound { path, reason } => {
            Either5::A(TestSourceFileNotFound::new(path, reason))
        }
        collect_error::TestSourceCollectError::DirectiveLocation {
            contract,
            function,
            reason,
        } => Either5::B(TestSourceDirectiveLocation::new(contract, function, reason)),
        collect_error::TestSourceCollectError::SourcePathNotProvided => {
            Either5::C(TestSourcePathNotProvided::new())
        }
        collect_error::TestSourceCollectError::UnsupportedSolcVersion { version } => {
            Either5::D(TestSourceUnsupportedSolcVersion::new(version.to_string()))
        }
        collect_error::TestSourceCollectError::SourceParseErrors { reasons } => {
            Either5::E(TestSourceParseErrors::new(reasons))
        }
    }
}

fn to_directive_problem(error: collect_error::InlineConfigError) -> InlineConfigDirectiveProblem {
    match error {
        collect_error::InlineConfigError::InvalidSyntax { line } => {
            Either6::A(InlineConfigInvalidSyntax::new(line))
        }
        collect_error::InlineConfigError::UnsupportedProfile { profile } => {
            Either6::B(InlineConfigUnsupportedProfile::new(profile))
        }
        collect_error::InlineConfigError::InvalidKey { key } => {
            Either6::C(InlineConfigInvalidKey::new(key))
        }
        collect_error::InlineConfigError::InvalidKeyForTestType { key, test_type } => {
            Either6::D(InlineConfigInvalidKeyForTestType::new(key, test_type))
        }
        collect_error::InlineConfigError::InvalidValue {
            key,
            value,
            expected,
        } => Either6::E(InlineConfigInvalidValue::new(
            key,
            value,
            expected.to_owned(),
        )),
        collect_error::InlineConfigError::DuplicateKey { key } => {
            Either6::F(InlineConfigDuplicateKey::new(key.clone()))
        }
    }
}

fn to_entry(item: collect_error::TestSourceErrorItem) -> TestSourceError {
    let source_name = item.source_name.to_string_lossy().into_owned();
    match item.problem {
        collect_error::TestSourceProblem::Source(error) => Either::A(TestSourceFileError::new(
            source_name,
            to_source_problem(error),
        )),
        collect_error::TestSourceProblem::Directive(
            collect_error::InlineConfigDirectiveError {
                contract,
                function,
                line,
                error,
            },
        ) => Either::B(InlineConfigDirectiveError::new(
            source_name,
            contract.clone(),
            function.clone(),
            line,
            to_directive_problem(error),
        )),
    }
}

/// Builds the error that rejects `runSolidityTests` when collection fails,
/// carrying the structured, located problems on the JS error as its
/// `testSourceErrors` property.
///
/// Must be called on the JS thread (it builds JS values). The reject path in
/// [`crate::context`] does so from the deferred's resolver. Falls back to a
/// plain message-only error if building the structured object fails.
pub(crate) fn to_napi_error(env: &Env, errors: collect_error::TestSourceErrors) -> napi::Error {
    fn build_structured_error(
        env: &Env,
        summary: &str,
        errors: collect_error::TestSourceErrors,
    ) -> napi::Result<napi::Error> {
        let mut error_object = env.create_error(napi::Error::from_reason(summary))?;
        let items: Vec<TestSourceError> = errors.into_items().into_iter().map(to_entry).collect();
        error_object.set("testSourceErrors", items)?;
        Ok(napi::Error::from(error_object.to_unknown()))
    }

    let summary = format!("Could not collect from the test sources:\n{errors}");
    build_structured_error(env, &summary, errors.clone())
        .unwrap_or_else(|_| napi::Error::from_reason(summary))
}
