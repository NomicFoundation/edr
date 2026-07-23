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

use edr_solidity_parser_slang::ImportResolver;
use slang_solidity_v2::compilation::CompilationUnit;

use super::{
    directives::{self, LocatedDirectiveError},
    error::{InlineConfigCollectError, InlineConfigErrorItem, InlineConfigProblem},
    natspec,
    parse::{locate_functions, locate_functions_in_unit, LocatedFunction},
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
pub(crate) type SourceOverrides = HashMap<String, Vec<FunctionOverride>>;

/// The outcome of collecting one source's inline configuration: the overrides
/// that parsed successfully, plus every problem found (at most one per test
/// function). Problems are accumulated rather than short-circuited so the run
/// can report them all together and abort up front.
pub(crate) struct SourceCollection {
    /// The successfully-parsed overrides, keyed by contract name.
    pub(crate) overrides: SourceOverrides,
    /// The problems found, in source order, each with its location.
    pub(crate) errors: Vec<InlineConfigErrorItem>,
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

/// Like [`collect_source`], but extracts from an already-built compilation
/// unit instead of reading and parsing the source itself. `file_id` is the id
/// the root file was added to the unit under (its on-disk path).
///
/// Used by the combined test-source collection
/// ([`crate::test_sources::collect_test_sources`]), which parses each source
/// once and extracts both its inline configuration and its EIP-712 struct
/// definitions from the same unit.
pub(crate) fn collect_source_from_unit(
    source: &Path,
    content: Arc<str>,
    unit: &CompilationUnit,
    file_id: &str,
) -> SourceCollection {
    let functions = locate_functions_in_unit(unit, file_id);
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
            Err(LocatedDirectiveError { offset, error }) => {
                // If the offending line itself cannot be located, report that
                // as a source-level problem — carrying the directive problem
                // in its message — rather than fabricating a line number.
                let problem = match line_of(&ast.source, offset) {
                    Ok(line) => InlineConfigProblem::Directive {
                        contract: contract_name.to_owned(),
                        function: function.function_name.clone(),
                        line,
                        error,
                    },
                    Err(line_error) => {
                        InlineConfigProblem::Source(InlineConfigCollectError::DirectiveLocation {
                            contract: contract_name.to_owned(),
                            function: function.function_name.clone(),
                            reason: format!("{line_error} (while reporting: {error})"),
                        })
                    }
                };
                errors.push(InlineConfigErrorItem {
                    source: source.to_path_buf(),
                    problem,
                });
            }
        }
    }

    (overrides, errors)
}

/// Why [`line_of`] could not resolve an offset to a line number. Either way,
/// the offsets handed across parsing stages are out of sync with the source
/// text, so a fabricated line number would point the user at the wrong place.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
enum LineOfError {
    /// The offset lies beyond the end of the source text.
    #[error("directive offset {offset} lies beyond the {source_len}-byte source")]
    OffsetOutOfBounds {
        /// The offending offset.
        offset: usize,
        /// The length of the source text.
        source_len: usize,
    },
    /// The line number does not fit in `u32`.
    #[error("line number {line} overflows u32")]
    LineOverflow {
        /// The 1-based line number that did not fit.
        line: usize,
    },
}

/// The 1-based line number of `offset` within `source`.
fn line_of(source: &str, offset: usize) -> Result<u32, LineOfError> {
    if offset > source.len() {
        return Err(LineOfError::OffsetOutOfBounds {
            offset,
            source_len: source.len(),
        });
    }
    let newlines = source
        .as_bytes()
        .iter()
        .take(offset)
        .filter(|&&byte| byte == b'\n')
        .count();
    u32::try_from(newlines + 1)
        .map_err(|_overflow| LineOfError::LineOverflow { line: newlines + 1 })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn line_of_counts_lines() {
        let source = "a\nb\nc";
        assert_eq!(line_of(source, 0), Ok(1));
        assert_eq!(line_of(source, 2), Ok(2));
        assert_eq!(line_of(source, source.len()), Ok(3));
    }

    #[test]
    fn line_of_rejects_out_of_bounds_offsets() {
        let source = "a\nb";
        assert_eq!(
            line_of(source, source.len() + 1),
            Err(LineOfError::OffsetOutOfBounds {
                offset: source.len() + 1,
                source_len: source.len(),
            })
        );
    }
}
