//! Composes the lower layers into a source's inline configuration.
//!
//! Given a source's text and solc version, this locates its functions
//! ([`super::parse`]), recovers each one's leading NatSpec
//! ([`super::natspec`]), parses the directives within
//! ([`super::directives`]), and groups the results per contract.

use std::{collections::HashMap, sync::Arc};

use semver::Version;

use super::{
    directives,
    error::InlineConfigError,
    natspec,
    parse::{locate_functions, LocatedFunction},
};
use crate::config::TestFunctionConfigOverride;

/// The inline configuration parsed for a single test function.
#[derive(Clone, Debug)]
pub struct FunctionOverride {
    /// The function name.
    pub function_name: String,
    /// The parsed configuration override.
    pub config: TestFunctionConfigOverride,
}

/// The fully-parsed inline configuration of every contract in one source that
/// declares any — each contract's overrides, or the error from a malformed
/// directive. Errors are kept per contract so one bad contract doesn't poison
/// its neighbours; a contract with no directives is simply absent.
pub(super) type SourceOverrides = HashMap<String, Result<Vec<FunctionOverride>, InlineConfigError>>;

/// Parses a source (its `text`, compiled with `version`) into the inline
/// configuration of every contract it declares.
pub(super) fn collect_source(text: Arc<str>, version: Version) -> SourceOverrides {
    let functions = locate_functions(&text, version);
    source_overrides(&SourceAst {
        source: text,
        functions,
    })
}

/// The structural information extracted from a single source file: its text and
/// the functions it declares (with the offset needed to recover their leading
/// NatSpec).
struct SourceAst {
    source: Arc<str>,
    functions: Vec<LocatedFunction>,
}

/// Parses the inline configuration of every contract in `ast` that declares a
/// directive, keyed by contract name. Contracts with no directives are omitted
/// (a query for them returns an empty vector); a malformed directive is
/// captured as that contract's error rather than failing the whole source.
fn source_overrides(ast: &SourceAst) -> SourceOverrides {
    let mut by_contract = SourceOverrides::new();

    for function in &ast.functions {
        if by_contract.contains_key(&function.contract_name) {
            continue;
        }
        match contract_overrides(ast, &function.contract_name) {
            // Nothing to cache; a query returns an empty vector for absent entries.
            Ok(overrides) if overrides.is_empty() => {}
            result => {
                by_contract.insert(function.contract_name.clone(), result);
            }
        }
    }

    by_contract
}

/// Parses the inline configuration of every test function in `contract_name`
/// within the already-parsed `ast`.
fn contract_overrides(
    ast: &SourceAst,
    contract_name: &str,
) -> Result<Vec<FunctionOverride>, InlineConfigError> {
    let mut overrides = Vec::new();

    for function in &ast.functions {
        if function.contract_name != contract_name {
            continue;
        }
        // Only test functions carry inline configuration. The recognized
        // prefixes mirror the runner's test-function classification
        // (`test*`, `invariant*`, `statefulFuzz*`).
        if !directives::is_test_function(&function.function_name) {
            continue;
        }

        let blocks = natspec::collect_natspec(&ast.source, function.node_start);
        if blocks.is_empty() {
            continue;
        }

        if let Some(config) =
            directives::parse_inline_config(&blocks, contract_name, &function.function_name)?
        {
            overrides.push(FunctionOverride {
                function_name: function.function_name.clone(),
                config,
            });
        }
    }

    Ok(overrides)
}
