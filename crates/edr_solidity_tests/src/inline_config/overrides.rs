//! Composes the lower layers into a source's inline configuration.
//!
//! Given a source file on disk and its solc version, this locates its contracts
//! and functions ([`super::parse`]), recovers each one's leading NatSpec
//! ([`super::natspec`]), parses the directives within
//! ([`super::directives`]), and groups the results per contract.

use std::{collections::HashMap, path::Path, sync::Arc};

use semver::Version;

use super::{
    directives::{self, DirectiveTarget, LocatedDirectiveError},
    error::{InlineConfigCollectError, InlineConfigErrorItem, InlineConfigProblem},
    natspec,
    parse::{locate_contracts, LocatedContract},
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

/// The inline configuration parsed for a single contract: the contract-level
/// configuration (from NatSpec above the contract definition, applying to every
/// test the contract runs) and the per-function overrides (from NatSpec above
/// each test function, taking per-key precedence over the contract level).
#[derive(Clone, Debug, Default)]
pub struct ContractInlineConfig {
    /// The contract-level configuration, if the contract declares any.
    pub contract: Option<TestFunctionConfigOverride>,
    /// The per-function overrides, in source order.
    pub functions: Vec<FunctionOverride>,
}

impl ContractInlineConfig {
    /// Whether neither the contract nor any of its functions declares inline
    /// configuration.
    pub fn is_empty(&self) -> bool {
        self.contract.is_none() && self.functions.is_empty()
    }
}

/// The successfully-parsed inline configuration of every contract in one source
/// that declares any, keyed by contract name. A contract with no directives is
/// simply absent; a contract whose directives were all malformed is likewise
/// absent (its problems live in [`SourceCollection::errors`]).
pub(super) type SourceOverrides = HashMap<String, ContractInlineConfig>;

/// The outcome of collecting one source's inline configuration: the overrides
/// that parsed successfully, plus every problem found (at most one per test
/// function, plus at most one per contract's own directives). Problems are
/// accumulated rather than short-circuited so the run can report them all
/// together and abort up front.
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
/// A failure to locate the source's contracts (an unsupported solc version)
/// becomes the collection's single (source-level) error; otherwise every
/// contract is parsed and its per-function problems accumulated.
pub(super) fn collect_source(
    source: &Path,
    root_path: &Path,
    content: Arc<str>,
    version: Version,
    import_resolver: &ImportResolver,
) -> SourceCollection {
    let contracts = match locate_contracts(root_path, version, import_resolver) {
        Ok(contracts) => contracts,
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

    let mut overrides = SourceOverrides::new();
    let mut errors = Vec::new();
    for located in &contracts {
        let (contract, contract_errors) = contract_overrides(source, &content, located);
        if !contract.is_empty() {
            overrides.insert(located.contract_name.clone(), contract);
        }
        errors.extend(contract_errors);
    }

    SourceCollection { overrides, errors }
}

/// Parses the inline configuration of `contract` — the contract-level
/// directives above its definition and the per-function directives above each
/// of its test functions — within the already-parsed `source_text`, returning
/// the successful overrides and the problems found (at most one per function,
/// plus at most one for the contract's own directives), each located at its
/// source line.
fn contract_overrides(
    source: &Path,
    source_text: &str,
    contract: &LocatedContract,
) -> (ContractInlineConfig, Vec<InlineConfigErrorItem>) {
    let mut config = ContractInlineConfig::default();
    let mut errors = Vec::new();

    let mut located_problem =
        |function: Option<&str>, LocatedDirectiveError { offset, error }: LocatedDirectiveError| {
            // If the offending line itself cannot be located, report that as a
            // source-level problem — carrying the directive problem in its
            // message — rather than fabricating a line number.
            let problem = match line_of(source_text, offset) {
                Ok(line) => InlineConfigProblem::Directive {
                    contract: contract.contract_name.clone(),
                    function: function.map(str::to_owned),
                    line,
                    error,
                },
                Err(line_error) => {
                    InlineConfigProblem::Source(InlineConfigCollectError::DirectiveLocation {
                        contract: contract.contract_name.clone(),
                        function: function.map(str::to_owned),
                        reason: format!("{line_error} (while reporting: {error})"),
                    })
                }
            };
            errors.push(InlineConfigErrorItem {
                source: source.to_path_buf(),
                problem,
            });
        };

    // Contract-level directives.
    let blocks = natspec::collect_natspec(source_text, contract.node_start);
    if !blocks.is_empty() {
        match directives::parse_inline_config(&blocks, DirectiveTarget::Contract) {
            Ok(parsed) => config.contract = parsed,
            Err(error) => located_problem(None, error),
        }
    }

    for function in &contract.functions {
        // Only test functions carry inline configuration. The recognized
        // prefixes mirror the runner's test-function classification
        // (`test*`, `invariant*`, `statefulFuzz*`).
        if !directives::is_test_function(&function.function_name) {
            continue;
        }

        let blocks = natspec::collect_natspec(source_text, function.node_start);
        if blocks.is_empty() {
            continue;
        }

        // Only the first problem in a given function is reported; parsing moves
        // on to the next function so every function's problems surface.
        match directives::parse_inline_config(
            &blocks,
            DirectiveTarget::Function(&function.function_name),
        ) {
            Ok(Some(parsed)) => config.functions.push(FunctionOverride {
                function_name: function.function_name.clone(),
                config: parsed,
            }),
            Ok(None) => {}
            Err(error) => located_problem(Some(&function.function_name), error),
        }
    }

    (config, errors)
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
