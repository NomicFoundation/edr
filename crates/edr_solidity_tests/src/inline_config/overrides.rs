//! Composes the lower layers into a source's inline configuration.
//!
//! Given a source's already-built compilation unit and its text, this locates
//! its contracts and functions ([`super::parse`]), recovers each one's leading
//! NatSpec ([`super::natspec`]), parses the directives within
//! ([`super::directives`]), and groups the results per contract.

use std::{collections::HashMap, path::Path};

use slang_solidity_v2::compilation::CompilationUnit;

use super::{
    directives::{self, DirectiveTarget, LocatedDirectiveError},
    error::{InlineConfigCollectError, InlineConfigDirectiveError, InlineConfigErrorItem},
    natspec,
    parse::{locate_contracts_in_unit, LocatedContract, LocatedFunction},
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

/// The inline configuration parsed for a single contract: the directives above
/// the contract definition, and those above each of its test functions.
#[derive(Clone, Debug, Default)]
pub struct ContractInlineConfig {
    /// The contract-level configuration, if the contract declares any. Applies
    /// to every test the contract runs; function-level overrides win per key.
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
/// absent — the whole collection fails with those problems instead.
pub(crate) type SourceOverrides = HashMap<String, ContractInlineConfig>;

/// Extracts the inline configuration of every contract in the already-built
/// `unit`'s file `file_id` (the id the root file was added under). `content`
/// is that file's text, which the NatSpec is recovered from; `source` names
/// the file in error reports (the solc source name the caller queries by).
///
/// Every problem found is accumulated — at most one per test function, plus at
/// most one per contract's own directives — rather than short-circuited, so the
/// run can report them all together and abort up front.
///
/// Used by the combined test-source collection
/// ([`crate::test_sources::collect_test_sources`]), which parses each source
/// once and extracts both its inline configuration and its EIP-712 struct
/// definitions from the same unit.
pub(crate) fn collect_source_overrides_from_unit(
    source: &Path,
    content: &str,
    unit: &CompilationUnit,
    file_id: &str,
) -> Result<SourceOverrides, Vec<InlineConfigErrorItem>> {
    source_overrides(source, content, &locate_contracts_in_unit(unit, file_id))
}

/// Parses the inline configuration of every contract in `contracts` that
/// declares a directive. Contracts with no directives are omitted from the
/// result (a query for them returns an empty configuration).
fn source_overrides(
    source: &Path,
    content: &str,
    contracts: &[LocatedContract],
) -> Result<SourceOverrides, Vec<InlineConfigErrorItem>> {
    let mut overrides = SourceOverrides::new();
    let mut errors = Vec::new();
    for located in contracts {
        let (contract, contract_errors) = contract_overrides(source, content, located);
        if !contract.is_empty() {
            overrides.insert(located.contract_name.clone(), contract);
        }
        errors.extend(contract_errors);
    }

    if errors.is_empty() {
        Ok(overrides)
    } else {
        Err(errors)
    }
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

    match collect_contract_level_directives(source_text, contract) {
        Ok(parsed) => config.contract = parsed,
        Err(error) => errors.push(located_problem(source, source_text, contract, None, error)),
    }

    for function in &contract.functions {
        match collect_function_level_directives(source_text, function) {
            Ok(Some(parsed)) => config.functions.push(FunctionOverride {
                function_name: function.function_name.clone(),
                config: parsed,
            }),
            Ok(None) => {}
            Err(error) => errors.push(located_problem(
                source,
                source_text,
                contract,
                Some(&function.function_name),
                error,
            )),
        }
    }

    (config, errors)
}

/// Parses the contract-level directives from the NatSpec above `contract`'s
/// definition, returning the configuration if any directives are declared.
fn collect_contract_level_directives(
    source_text: &str,
    contract: &LocatedContract,
) -> Result<Option<TestFunctionConfigOverride>, LocatedDirectiveError> {
    let blocks = natspec::collect_natspec(source_text, contract.node_start);
    if blocks.is_empty() {
        return Ok(None);
    }
    directives::parse_inline_config(&blocks, DirectiveTarget::Contract)
}

/// Parses the per-function directives from the NatSpec above `function`,
/// returning its override if it is a test function that declares any. Only the
/// first problem in a given function is reported.
fn collect_function_level_directives(
    source_text: &str,
    function: &LocatedFunction,
) -> Result<Option<TestFunctionConfigOverride>, LocatedDirectiveError> {
    // Only test functions carry inline configuration. The recognized prefixes
    // mirror the runner's test-function classification (`test*`, `invariant*`,
    // `statefulFuzz*`).
    if !directives::is_test_function(&function.function_name) {
        return Ok(None);
    }

    let blocks = natspec::collect_natspec(source_text, function.node_start);
    if blocks.is_empty() {
        return Ok(None);
    }

    directives::parse_inline_config(&blocks, DirectiveTarget::Function(&function.function_name))
}

/// Locates a directive problem at its source line, in `contract` and (for a
/// function-level directive) `function`. If the offending line itself cannot be
/// located, reports that as a source-level problem — carrying the directive
/// problem in its message — rather than fabricating a line number.
fn located_problem(
    source: &Path,
    source_text: &str,
    contract: &LocatedContract,
    function: Option<&str>,
    LocatedDirectiveError { offset, error }: LocatedDirectiveError,
) -> InlineConfigErrorItem {
    let problem = match line_of(source_text, offset) {
        Ok(line) => InlineConfigDirectiveError {
            contract: contract.contract_name.clone(),
            function: function.map(str::to_owned),
            line,
            error,
        }
        .into(),
        Err(line_error) => InlineConfigCollectError::DirectiveLocation {
            contract: contract.contract_name.clone(),
            function: function.map(str::to_owned),
            reason: format!("{line_error} (while reporting: {error})"),
        }
        .into(),
    };
    InlineConfigErrorItem {
        source_name: source.to_path_buf(),
        problem,
    }
}

/// Why [`line_of`] could not resolve an offset to a line number. Either way,
/// the offsets handed across parsing stages are out of sync with the source
/// text, so a fabricated line number would point the user at the wrong place.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub(crate) enum LineOfError {
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
pub(crate) fn line_of(source: &str, offset: usize) -> Result<u32, LineOfError> {
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
