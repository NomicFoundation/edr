//! Structural extraction of contracts and functions via Slang's compilation
//! unit.
//!
//! We build a full [`CompilationUnit`] over the on-disk root file — resolving
//! imports (see [`super::resolver`]) and reading them from disk — then walk the
//! root file's resolved AST for contract and function positions. The NatSpec
//! text itself is recovered from the raw source by
//! [`super::natspec::collect_natspec`], which scans backwards from each
//! definition.
//!
//! [`CompilationUnit`]: slang_solidity_v2::compilation::CompilationUnit

use std::path::Path;

use semver::Version;
use slang_solidity_v2::{
    ast::{ContractMember, SourceUnitMember},
    compilation::CompilationBuilder,
    utils::{FromSemverError, LanguageVersion},
};

use super::{
    error::InlineConfigCollectError,
    resolver::{ImportResolver, SourceProvider},
};

/// A function definition located in the source, with the offset needed to
/// recover its leading NatSpec.
#[derive(Clone, Debug)]
pub struct LocatedFunction {
    /// The function name.
    pub function_name: String,
    /// Byte offset where the function definition starts (its `function`
    /// keyword). The leading NatSpec is recovered by scanning backwards from
    /// here.
    pub node_start: usize,
}

/// A contract definition located in the source, with the offset needed to
/// recover its leading NatSpec, together with the functions it declares
/// directly (inherited members live with their declaring contract).
#[derive(Clone, Debug)]
pub struct LocatedContract {
    /// The contract name.
    pub contract_name: String,
    /// Byte offset where the contract definition starts (its `contract`
    /// keyword, or `abstract` for abstract contracts). The leading NatSpec is
    /// recovered by scanning backwards from here.
    pub node_start: usize,
    /// The functions declared directly in the contract, in source order.
    pub functions: Vec<LocatedFunction>,
}

/// Maps a solc [`Version`] to a Slang [`LanguageVersion`]; clamping versions
/// newer than Slang supports down to its latest grammar.
fn to_language_version(solc_version: Version) -> Result<LanguageVersion, FromSemverError> {
    // Fall back to the latest Slang grammar for any solc version newer than what
    // Slang supports.
    let latest: Version = LanguageVersion::LATEST.into();
    if solc_version > latest {
        Ok(LanguageVersion::LATEST)
    } else {
        LanguageVersion::try_from(solc_version)
    }
}

/// Parses the file at `root_path` (with its imports resolved by
/// `import_resolver` and read from disk) and returns every contract definition
/// together with its functions and the offsets required to recover their
/// leading NatSpec.
///
/// Builds a full compilation unit — resolving imports and running IR and
/// semantic analysis — then reads the root file's AST. Unresolvable imports
/// degrade gracefully: the root file's contracts are still recovered.
///
/// Fails if `version` maps to no supported Slang grammar.
pub fn locate_contracts(
    root_path: &Path,
    version: Version,
    import_resolver: &ImportResolver,
) -> Result<Vec<LocatedContract>, InlineConfigCollectError> {
    let mut builder = CompilationBuilder::create(
        to_language_version(version)?,
        SourceProvider::new(import_resolver),
    );
    let file_id = root_path.to_string_lossy().into_owned();
    builder.add_file(file_id.clone());
    let unit = builder.build();

    let Some(file) = unit.file(&file_id) else {
        return Ok(Vec::new());
    };

    let mut contracts = Vec::new();
    for member in file.ast().members().iter() {
        let SourceUnitMember::ContractDefinition(contract) = member else {
            continue;
        };

        let mut functions = Vec::new();
        for contract_member in contract.members().iter() {
            let ContractMember::FunctionDefinition(function) = contract_member else {
                continue;
            };
            let Some(name) = function.name() else {
                continue;
            };
            functions.push(LocatedFunction {
                function_name: name.name(),
                node_start: function.get_text_range().start,
            });
        }

        contracts.push(LocatedContract {
            contract_name: contract.name().name(),
            node_start: contract.get_text_range().start,
            functions,
        });
    }

    Ok(contracts)
}

#[cfg(test)]
mod tests {
    use std::io::Write as _;

    use super::*;

    #[test]
    fn locates_contracts_and_functions_with_offsets() {
        let source = "// SPDX-License-Identifier: MIT\npragma solidity ^0.8.0;\n\n/// forge-config: default.fuzz.runs = 9\ncontract C {\n    uint256 internal value;\n\n    /// forge-config: default.fuzz.runs = 5\n    function testFoo(uint256 x) public {}\n}\n";
        let mut file = tempfile::Builder::new()
            .suffix(".sol")
            .tempfile()
            .expect("temp file");
        file.write_all(source.as_bytes()).expect("write source");

        let version = Version::new(0, 8, 0);
        let contracts = locate_contracts(file.path(), version, &ImportResolver::default())
            .expect("0.8.0 is supported");
        assert_eq!(contracts.len(), 1, "contracts: {contracts:#?}");

        let contract = &contracts[0];
        assert_eq!(contract.contract_name, "C");

        // `node_start` is the `contract` keyword, excluding leading comments.
        assert!(source
            .get(contract.node_start..)
            .unwrap()
            .starts_with("contract C"));

        // The backward scan recovers the contract-level directive without
        // picking up the pragma or license comment.
        let blocks = crate::inline_config::natspec::collect_natspec(source, contract.node_start);
        assert!(blocks.iter().any(|block| block.text.contains("runs = 9")));
        assert!(blocks.iter().all(|block| !block.text.contains("pragma")));

        assert_eq!(contract.functions.len(), 1, "{:#?}", contract.functions);
        let function = &contract.functions[0];
        assert_eq!(function.function_name, "testFoo");

        // `node_start` is the `function` keyword, excluding leading comments.
        assert!(source
            .get(function.node_start..)
            .unwrap()
            .starts_with("function testFoo"));

        // The backward scan recovers the directive without picking up the
        // preceding state variable.
        let blocks = crate::inline_config::natspec::collect_natspec(source, function.node_start);
        assert!(blocks.iter().any(|block| block.text.contains("runs = 5")));
        assert!(blocks.iter().all(|block| !block.text.contains("value")));
    }
}
