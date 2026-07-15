//! Surfaces ill-formed inline configuration (`forge-config:`/
//! `hardhat-config:` NatSpec directives) as a structured JS error.

use edr_solidity_tests::inline_config::{InlineConfigErrorItem, InlineConfigErrors};
use napi::{Env, JsValue};
use napi_derive::napi;

/// A single ill-formed inline-config directive, located so the user can find
/// and fix it. Attached to the rejected `runSolidityTests` promise as the
/// `inlineConfigErrors` array on the thrown error.
#[napi(object)]
pub struct InlineConfigError {
    /// The solc source name the problem was found in (e.g.
    /// `project/test/Foo.t.sol`).
    pub source_name: String,
    /// The contract the offending directive belongs to, if known.
    pub contract: Option<String>,
    /// The test function the offending directive belongs to, if known.
    pub function: Option<String>,
    /// The 1-based line of the offending directive within the source, if known.
    pub line: Option<u32>,
    /// A human-readable description of the problem.
    pub message: String,
}

impl From<&InlineConfigErrorItem> for InlineConfigError {
    fn from(item: &InlineConfigErrorItem) -> Self {
        Self {
            source_name: item.source.to_string_lossy().into_owned(),
            contract: item.contract.clone(),
            function: item.function.clone(),
            line: item.line,
            message: item.message.clone(),
        }
    }
}

/// Builds the error to reject `runSolidityTests` when inline-config validation
/// fails, carrying the structured, located problems on the JS error as its
/// `inlineConfigErrors` property.
///
/// Must be called on the JS thread (it builds JS values); the reject path in
/// [`crate::context`] does so from the deferred's resolver. Falls back to a
/// plain message-only error if building the structured object fails.
pub(crate) fn to_napi_error(env: &Env, errors: &InlineConfigErrors) -> napi::Error {
    build_structured_error(env, errors)
        .unwrap_or_else(|_| napi::Error::from_reason(summary(errors)))
}

fn build_structured_error(env: &Env, errors: &InlineConfigErrors) -> napi::Result<napi::Error> {
    let mut error_object = env.create_error(napi::Error::from_reason(summary(errors)))?;
    let items: Vec<InlineConfigError> =
        errors.items().iter().map(InlineConfigError::from).collect();
    error_object.set("inlineConfigErrors", items)?;
    Ok(napi::Error::from(error_object.to_unknown()))
}

fn summary(errors: &InlineConfigErrors) -> String {
    format!("Found invalid inline configuration in test sources:\n{errors}")
}
