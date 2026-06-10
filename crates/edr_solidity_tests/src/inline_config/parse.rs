//! Structural extraction of contracts and functions via Slang.
//!
//! Slang gives us the byte ranges of contract and function definitions; the
//! NatSpec text itself is recovered from the raw source by
//! [`super::natspec::collect_natspec`], which scans backwards from each
//! function and is bounded below by the contract's start.

use semver::Version;
use slang_solidity_v2::{
    ast::{ContractMember, SourceUnitMember},
    compilation::{CompilationBuilder, CompilationBuilderConfig},
    utils::LanguageVersion,
};

/// The synthetic file id used when compiling a single in-memory source.
const FILE_ID: &str = "source.sol";

/// A function definition located in the source, with the offset needed to
/// recover its leading NatSpec.
#[derive(Clone, Debug)]
pub struct LocatedFunction {
    /// The name of the enclosing contract.
    pub contract_name: String,
    /// The function name.
    pub function_name: String,
    /// Byte offset where the function definition starts (its `function`
    /// keyword). The leading NatSpec is recovered by scanning backwards from
    /// here.
    pub node_start: usize,
}

/// Single-file compilation config: serves `source` for the root id and reports
/// every import as unresolved (their diagnostics are ignored — we only need the
/// root file's AST).
struct SingleFileConfig<'a> {
    source: &'a str,
}

impl CompilationBuilderConfig for SingleFileConfig<'_> {
    fn read_file(&mut self, file_id: &str) -> Result<String, String> {
        if file_id == FILE_ID {
            Ok(self.source.to_owned())
        } else {
            Err(format!("unavailable file: {file_id}"))
        }
    }

    fn resolve_import(
        &mut self,
        _source_file_id: &str,
        _import_path: &str,
    ) -> Result<String, String> {
        // We only need the root file's structural AST, so leave every import
        // unresolved. Resolving to a file we can't supply would create a
        // dangling import edge and panic Slang's semantic binder.
        Err("imports are not resolved for inline-config parsing".to_owned())
    }
}

/// Maps a solc version to the closest Slang language version, falling back to
/// the latest supported version when the exact version is unavailable.
fn to_language_version(version: Version) -> LanguageVersion {
    LanguageVersion::try_from(version).unwrap_or(LanguageVersion::LATEST)
}

/// Parses `source` and returns every function definition together with the
/// offset required to recover its leading NatSpec.
pub fn locate_functions(source: &str, version: Version) -> Vec<LocatedFunction> {
    let mut builder =
        CompilationBuilder::create(to_language_version(version), SingleFileConfig { source });
    builder.add_file(FILE_ID.to_owned());
    let unit = builder.build();

    let Some(file) = unit.file(FILE_ID) else {
        return Vec::new();
    };

    let mut functions = Vec::new();
    for member in file.ast().members().iter() {
        let SourceUnitMember::ContractDefinition(contract) = member else {
            continue;
        };
        let contract_name = contract.name().name();

        for contract_member in contract.members().iter() {
            let ContractMember::FunctionDefinition(function) = contract_member else {
                continue;
            };
            let Some(name) = function.name() else {
                continue;
            };
            functions.push(LocatedFunction {
                contract_name: contract_name.clone(),
                function_name: name.name(),
                node_start: function.get_text_range().start,
            });
        }
    }

    functions
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn locates_functions_with_offsets() {
        let source = "// SPDX-License-Identifier: MIT\npragma solidity ^0.8.0;\n\ncontract C {\n    uint256 internal value;\n\n    /// forge-config: default.fuzz.runs = 5\n    function testFoo(uint256 x) public {}\n}\n";
        let version = Version::new(0, 8, 0);
        let functions = locate_functions(source, version.clone());
        assert_eq!(functions.len(), 1, "functions: {functions:#?}");

        let function = &functions[0];
        assert_eq!(function.contract_name, "C");
        assert_eq!(function.function_name, "testFoo");

        // `node_start` is the `function` keyword, excluding leading comments.
        assert!(source
            .get(function.node_start..)
            .unwrap()
            .starts_with("function testFoo"));

        // The backward scan recovers the directive without picking up the
        // preceding state variable.
        let blocks = crate::inline_config::natspec::collect_natspec(source, function.node_start);
        assert!(blocks
            .iter()
            .any(|block| block.text.contains("forge-config")));
        assert!(blocks.iter().all(|block| !block.text.contains("value")));
    }
}
