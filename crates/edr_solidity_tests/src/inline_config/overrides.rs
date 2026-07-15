//! Composes the lower layers into a source's inline configuration.
//!
//! Given a source file on disk and its solc version, this locates its functions
//! ([`super::parse`]), recovers each one's leading NatSpec
//! ([`super::natspec`]), parses the directives within
//! ([`super::directives`]), and groups the results per contract.

use std::{
    collections::{HashMap, HashSet},
    path::Path,
    sync::Arc,
};

use semver::Version;

use super::{
    directives,
    error::InlineConfigError,
    natspec,
    parse::{locate_functions, LocatedFunction},
    resolver::ImportResolver,
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

/// The fully-parsed inline configuration of every contract in one source, keyed
/// by contract name. A contract that declares no inline configuration is
/// omitted (a query for it returns an empty vector).
pub(super) type SourceOverrides = HashMap<String, Vec<FunctionOverride>>;

/// Parses the file at `root_path` (its `content`, compiled with `version`) into
/// the inline configuration of every contract it declares. Its imports are
/// resolved by `import_resolver` and read from disk.
///
/// A malformed directive fails the whole source (and, in turn, the whole run).
pub(super) fn collect_source(
    root_path: &Path,
    content: Arc<str>,
    version: Version,
    import_resolver: &ImportResolver,
) -> Result<SourceOverrides, InlineConfigError> {
    let functions = locate_functions(root_path, version, import_resolver)?;
    source_overrides(&SourceAst {
        source: content,
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

/// Parses the inline configuration of every contract in `ast`, keyed by
/// contract name. Contracts with no directives are omitted (a query returns an
/// empty vector). A malformed directive short-circuits with an error.
fn source_overrides(ast: &SourceAst) -> Result<SourceOverrides, InlineConfigError> {
    let mut by_contract = SourceOverrides::new();
    let mut seen = HashSet::new();

    for function in &ast.functions {
        if !seen.insert(function.contract_name.as_str()) {
            continue;
        }
        let overrides = contract_overrides(ast, &function.contract_name)?;
        if !overrides.is_empty() {
            by_contract.insert(function.contract_name.clone(), overrides);
        }
    }

    Ok(by_contract)
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
