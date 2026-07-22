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
    directives::{self, LocatedDirectiveError},
    error::{InlineConfigErrorItem, InlineConfigProblem},
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

/// The successfully-parsed inline configuration of every contract in one source
/// that declares any, keyed by contract name. A contract with no directives is
/// simply absent; a contract whose directives were all malformed is likewise
/// absent (its problems live in [`SourceCollection::errors`]).
pub(super) type SourceOverrides = HashMap<String, Vec<FunctionOverride>>;

/// The outcome of collecting one source's inline configuration: the overrides
/// that parsed successfully, plus every problem found (at most one per test
/// function). Problems are accumulated rather than short-circuited so the run
/// can report them all together and abort up front.
pub(super) struct SourceCollection {
    /// The successfully-parsed overrides, keyed by contract name.
    pub(super) overrides: SourceOverrides,
    /// The problems found, in source order, each with its location.
    pub(super) errors: Vec<InlineConfigErrorItem>,
}

/// Parses the file at `root_path` (its `content`, compiled with `version`) into
/// the inline configuration of every contract it declares. Its imports are
/// resolved by `import_resolver` and read from disk. `source` names the file
/// in error reports (the solc source name the caller queries by).
///
/// A failure to locate the source's functions (an unsupported solc version)
/// becomes the collection's single (source-level) error; otherwise every
/// contract is parsed and its per-function problems accumulated.
pub(super) fn collect_source(
    source: &Path,
    root_path: &Path,
    content: Arc<str>,
    version: Version,
    import_resolver: &ImportResolver,
) -> SourceCollection {
    let functions = match locate_functions(root_path, version, import_resolver) {
        Ok(functions) => functions,
        Err(error) => {
            return SourceCollection {
                overrides: SourceOverrides::new(),
                errors: vec![InlineConfigErrorItem {
                    source: source.to_path_buf(),
                    problem: InlineConfigProblem::Source(error),
                }],
            };
        }
    };
    source_overrides(
        source,
        &SourceAst {
            source: content,
            functions,
        },
    )
}

/// The structural information extracted from a single source file: its text and
/// the functions it declares (with the offset needed to recover their leading
/// NatSpec).
struct SourceAst {
    source: Arc<str>,
    functions: Vec<LocatedFunction>,
}

/// Parses the inline configuration of every contract in `ast` that declares a
/// directive. Contracts with no directives are omitted from
/// [`SourceCollection::overrides`] (a query for them returns an empty vector);
/// malformed directives are accumulated into [`SourceCollection::errors`]
/// rather than failing the source.
fn source_overrides(source: &Path, ast: &SourceAst) -> SourceCollection {
    let mut overrides = SourceOverrides::new();
    let mut errors = Vec::new();
    let mut seen = HashSet::new();

    for function in &ast.functions {
        if !seen.insert(function.contract_name.as_str()) {
            continue;
        }
        let (contract, contract_errors) = contract_overrides(source, ast, &function.contract_name);
        if !contract.is_empty() {
            overrides.insert(function.contract_name.clone(), contract);
        }
        errors.extend(contract_errors);
    }

    SourceCollection { overrides, errors }
}

/// Parses the inline configuration of every test function in `contract_name`
/// within the already-parsed `ast`, returning the successful overrides and the
/// problems found (at most one per function), each located at its source line.
fn contract_overrides(
    source: &Path,
    ast: &SourceAst,
    contract_name: &str,
) -> (Vec<FunctionOverride>, Vec<InlineConfigErrorItem>) {
    let mut overrides = Vec::new();
    let mut errors = Vec::new();

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

        // Only the first problem in a given function is reported; parsing moves
        // on to the next function so every function's problems surface.
        match directives::parse_inline_config(&blocks, &function.function_name) {
            Ok(Some(config)) => overrides.push(FunctionOverride {
                function_name: function.function_name.clone(),
                config,
            }),
            Ok(None) => {}
            Err(LocatedDirectiveError { offset, error }) => errors.push(InlineConfigErrorItem {
                source: source.to_path_buf(),
                problem: InlineConfigProblem::Directive {
                    contract: contract_name.to_owned(),
                    function: function.function_name.clone(),
                    line: line_of(&ast.source, offset),
                    error,
                },
            }),
        }
    }

    (overrides, errors)
}

/// The 1-based line number of `offset` within `source`.
fn line_of(source: &str, offset: usize) -> u32 {
    let end = offset.min(source.len());
    let newlines = source
        .as_bytes()
        .iter()
        .take(end)
        .filter(|&&byte| byte == b'\n')
        .count();
    u32::try_from(newlines + 1).unwrap_or(u32::MAX)
}
