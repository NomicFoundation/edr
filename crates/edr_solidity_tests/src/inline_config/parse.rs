//! Structural extraction of contracts and functions via Slang's compilation
//! unit.
//!
//! We build one [`CompilationUnit`] over every root file sharing a language
//! version — resolving imports (see [`super::resolver`]) and reading them from
//! disk — then walk each root file's resolved AST for contract and function
//! positions. Roots that share imports (e.g. `forge-std`'s `Test.sol`) thus
//! parse and analyze them once. The NatSpec text itself is recovered from the
//! raw source by [`super::natspec::collect_natspec`], which scans backwards
//! from each definition.

use std::{collections::HashMap, path::Path};

use semver::Version;
use slang_solidity_v2::{
    ast::{ContractMember, SourceUnitMember},
    compilation::{CompilationBuilder, CompilationUnit},
    utils::{FromSemverError, LanguageVersion},
};

use super::resolver::{ImportResolver, SourceProvider};

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
pub(super) fn to_language_version(
    solc_version: Version,
) -> Result<LanguageVersion, FromSemverError> {
    // Fall back to the latest Slang grammar for any solc version newer than what
    // Slang supports.
    let latest: Version = LanguageVersion::LATEST.into();
    if solc_version > latest {
        Ok(LanguageVersion::LATEST)
    } else {
        LanguageVersion::try_from(solc_version)
    }
}

/// The Slang file ID of the root file at `root_path`: the path itself, which
/// is what [`SourceProvider`] reads from disk and what relative imports are
/// resolved against.
pub(super) fn root_file_id(root_path: &Path) -> String {
    root_path.to_string_lossy().into_owned()
}

/// Builds a single [`CompilationUnit`] over every root in `roots`, all
/// compiled with `language_version`. Each root's imports are resolved by
/// `import_resolver` and read from disk; the roots themselves are read from
/// `roots` (keyed by [`root_file_id`]) rather than from disk again.
///
/// Runs Slang's IR and semantic analysis once over the union of the roots and
/// their (deduplicated) imports. Unresolvable imports degrade gracefully: they
/// are recorded as diagnostics and every root's contracts are still recovered.
pub(super) fn compile_roots(
    language_version: LanguageVersion,
    roots: &HashMap<String, &str>,
    import_resolver: &ImportResolver,
) -> CompilationUnit {
    let mut builder = CompilationBuilder::create(
        language_version,
        SourceProvider::new(import_resolver, roots),
    );
    for file_id in roots.keys() {
        builder.add_file(file_id.clone());
    }
    builder.build()
}

/// Returns every contract definition in the file `file_id` of `unit`, together
/// with its functions and the offsets required to recover their leading
/// NatSpec. Returns no contracts if the file is not part of the unit.
pub(super) fn locate_contracts(unit: &CompilationUnit, file_id: &str) -> Vec<LocatedContract> {
    let Some(file) = unit.file(file_id) else {
        return Vec::new();
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

    contracts
}

#[cfg(test)]
mod tests {
    use super::*;

    const SOURCE: &str = "// SPDX-License-Identifier: MIT\npragma solidity ^0.8.0;\n\n/// forge-config: default.fuzz.runs = 9\ncontract C {\n    uint256 internal value;\n\n    /// forge-config: default.fuzz.runs = 5\n    function testFoo(uint256 x) public {}\n}\n";

    /// Compiles `roots` (file ID to preloaded content) with solc 0.8.0.
    fn compile(roots: &HashMap<String, &str>) -> CompilationUnit {
        let version = to_language_version(Version::new(0, 8, 0)).expect("0.8.0 is supported");
        compile_roots(version, roots, &ImportResolver::default())
    }

    #[test]
    fn locates_contracts_and_functions_with_offsets() {
        let source = SOURCE;
        // The root is served from the preloaded contents, so no file need exist
        // at its path.
        let file_id = root_file_id(Path::new("/project/test/C.t.sol"));
        let unit = compile(&HashMap::from([(file_id.clone(), source)]));

        let contracts = locate_contracts(&unit, &file_id);
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

    #[test]
    fn one_unit_serves_multiple_roots() {
        let a = root_file_id(Path::new("/project/test/A.t.sol"));
        let b = root_file_id(Path::new("/project/test/B.t.sol"));
        let b_source = SOURCE.replace("contract C", "contract D");
        let unit = compile(&HashMap::from([
            (a.clone(), SOURCE),
            (b.clone(), b_source.as_str()),
        ]));

        assert_eq!(locate_contracts(&unit, &a)[0].contract_name, "C");
        assert_eq!(locate_contracts(&unit, &b)[0].contract_name, "D");
        assert!(locate_contracts(&unit, "/project/test/Missing.t.sol").is_empty());
    }

    #[test]
    fn clamps_newer_solc_versions_to_the_latest_grammar() {
        assert_eq!(
            to_language_version(Version::new(99, 0, 0)),
            Ok(LanguageVersion::LATEST)
        );
        assert_eq!(
            to_language_version(Version::new(0, 8, 0)),
            LanguageVersion::try_from(Version::new(0, 8, 0))
        );
    }
}
