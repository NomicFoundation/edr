//! Structural extraction of contracts and functions via Slang's compilation
//! unit.
//!
//! We build a full [`CompilationUnit`] over the on-disk root file — resolving
//! imports (see [`edr_solidity_parser_slang::ImportResolver`]) and reading
//! them from disk — then walk the
//! root file's resolved AST for contract and function positions. The NatSpec
//! text itself is recovered from the raw source by
//! [`super::natspec::collect_natspec`], which scans backwards from each
//! function.
//!
//! [`CompilationUnit`]: slang_solidity_v2::compilation::CompilationUnit

use slang_solidity_v2::{
    ast::{ContractMember, SourceUnitMember},
    compilation::CompilationUnit,
};

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

/// Walks the already-built `unit`'s file `file_id` (the id the root was added
/// under) and returns every function definition together with the offset
/// required to recover its leading NatSpec. A file id missing from the unit
/// (e.g. the root file could not be read) yields no functions.
pub(crate) fn locate_functions_in_unit(
    unit: &CompilationUnit,
    file_id: &str,
) -> Vec<LocatedFunction> {
    let Some(file) = unit.file(file_id) else {
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
    use std::{io::Write as _, path::Path};

    use edr_solidity_parser_slang::{build_compilation_unit, ImportResolver};
    use semver::Version;

    use super::*;
    use crate::inline_config::error::InlineConfigCollectError;

    /// Parses the file at `root_path` (with its imports resolved by
    /// `import_resolver` and read from disk) and returns every function
    /// definition together with the offset required to recover its leading
    /// NatSpec.
    ///
    /// Builds a full compilation unit — resolving imports and running IR and
    /// semantic analysis — then reads the root file's AST. Unresolvable imports
    /// degrade gracefully: the root file's functions are still recovered.
    ///
    /// Fails if `version` maps to no supported Slang grammar.
    pub fn locate_functions(
        root_path: &Path,
        version: Version,
        import_resolver: &ImportResolver,
    ) -> Result<Vec<LocatedFunction>, InlineConfigCollectError> {
        let unit = build_compilation_unit(root_path, version, import_resolver)?;
        let file_id = root_path.to_string_lossy().into_owned();

        Ok(locate_functions_in_unit(&unit, &file_id))
    }

    #[test]
    fn locates_functions_with_offsets() {
        let source = "// SPDX-License-Identifier: MIT\npragma solidity ^0.8.0;\n\ncontract C {\n    uint256 internal value;\n\n    /// forge-config: default.fuzz.runs = 5\n    function testFoo(uint256 x) public {}\n}\n";
        let mut file = tempfile::Builder::new()
            .suffix(".sol")
            .tempfile()
            .expect("temp file");
        file.write_all(source.as_bytes()).expect("write source");

        let version = Version::new(0, 8, 0);
        let functions = locate_functions(file.path(), version, &ImportResolver::default())
            .expect("0.8.0 is supported");
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
