//! Processes the Solidity compiler standard JSON[^1] input and output AST and
//! creates the source model used to perform the stack trace decoding.
//!
//! [^1]: See <https://docs.soliditylang.org/en/latest/using-the-compiler.html#compiler-input-and-output-json-description>.
use std::sync::Arc;

use anyhow::{self, Context as _};
use edr_primitives::hex;
use parking_lot::RwLock;

use crate::{
    artifacts::{CompilerOutput, CompilerOutputContract},
    build_model::{BuildModel, Contract, ContractMetadata, CustomError},
    contracts_identifier::IdentifiedContract,
    debug_info::CompilerArtifact,
    library_utils::{get_library_address_positions, normalize_compiler_output_bytecode},
};

/// First Solc version supported for stack trace generation
pub const FIRST_SOLC_VERSION_SUPPORTED: semver::Version = semver::Version::new(0, 5, 1);

/// For the Solidity compiler version and its standard JSON input and output,
/// creates the source model, decodes the bytecode, and links them to the
/// source files. The producing compiler is expressed by the concrete
/// `ArtifactT` (or `Box<dyn CompilerArtifact>` after the factory) — no
/// external tag is threaded in.tCom
pub fn populate_decoded_bytecodes<BuildModelT: BuildModel>(
    solc_version: String,
    build_model: BuildModelT,
    compiler_output: &CompilerOutput<<BuildModelT as BuildModel>::Artifact>,
) -> anyhow::Result<Vec<IdentifiedContract>> {
    let contracts = decode_bytecodes(solc_version, &compiler_output, &build_model)?;

    correct_selectors(&contracts, compiler_output)?;

    Ok(contracts)
}

fn correct_selectors<ArtifactT: CompilerArtifact>(
    contracts: &[IdentifiedContract],
    compiler_output: &CompilerOutput<ArtifactT>,
) -> anyhow::Result<()> {
    for identified in contracts
        .iter()
        .filter(|c| !c.contract_metadata.is_deployment)
    {
        let mut contract = identified.contract_metadata.contract.write();
        // Fetch the method identifiers for the contract from the compiler output
        let method_identifiers = match compiler_output
            .contracts
            .get(&contract.location.file()?.read().source_name)
            .and_then(|file| file.get(&contract.name))
            .map(|contract| &contract.evm.method_identifiers)
        {
            Some(ids) => ids,
            None => continue,
        };

        for (signature, hex_selector) in method_identifiers {
            let function_name = signature.split('(').next().unwrap_or("");
            let selector = hex::decode(hex_selector)
                .with_context(|| format!("Failed to decode hex: {hex_selector:?}"))?;

            let contract_function = contract.get_function_from_selector(&selector);

            if contract_function.is_some() {
                continue;
            }

            // NOTE: This code path is not covered by any of the existing tests.
            // Let's create a stack trace that exercises that code path or
            // let's remove it if/when we adapt our model to also properly
            // support ABI v2.
            let fixed_selector =
                contract.correct_selector(function_name.to_string(), selector.clone());

            if !fixed_selector {
                return Err(anyhow::anyhow!(
                    "Failed to fix up the selector for one or more implementations of {}#{}. Hardhat Network can automatically fix this problem if you don't use function overloading.",
                    contract.name,
                    function_name
                ));
            }
        }
    }
    Ok(())
}

fn decode_evm_bytecode<BuildModelT: BuildModel>(
    contract: Arc<RwLock<Contract>>,
    solc_version: String,
    is_deployment: bool,
    artifact: &<BuildModelT as BuildModel>::Artifact,
    build_model: &BuildModelT,
) -> anyhow::Result<IdentifiedContract> {
    let library_address_positions = get_library_address_positions(artifact);

    let immutable_references = artifact
        .immutable_references()
        .map(|refs| refs.values().flatten().copied().collect::<Vec<_>>())
        .unwrap_or_default();

    let normalized_code = normalize_compiler_output_bytecode(
        artifact.object().to_owned(),
        &library_address_positions,
    )
    .with_context(|| format!("Failed to decode hex: {:?}", artifact.object()))?;

    let section = if is_deployment {
        "evm.bytecode"
    } else {
        "evm.deployedBytecode"
    };
    let instructions = build_model
        .decode_instructions(artifact, &normalized_code, is_deployment)
        .with_context(|| format!("failed to decode debug-info for {section}"))?;

    Ok(IdentifiedContract {
        contract_metadata: Arc::new(ContractMetadata::new(
            contract,
            is_deployment,
            normalized_code,
            instructions,
            library_address_positions,
            immutable_references,
            solc_version,
        )),
        trace_strategy: artifact.trace_strategy(),
    })
}

fn decode_bytecodes<BuildModelT: BuildModel>(
    solc_version: String,
    compiler_output: &CompilerOutput<<BuildModelT as BuildModel>::Artifact>,
    build_model: &BuildModelT,
) -> anyhow::Result<Vec<IdentifiedContract>> {
    let mut bytecodes = Vec::new();

    for contract in build_model.contracts() {
        let contract_rc = contract.clone();

        let contract_evm_output = {
            let mut contract = contract.write();

            let contract_file = &contract.location.file()?.read().source_name.clone();
            let CompilerOutputContract { evm, abi } = &compiler_output
                .contracts
                .get(contract_file)
                .expect("contract_file should exist in contracts")
                .get(&contract.name)
                .expect("contract.name should exist in contract_file");

            for item in abi {
                if item.r#type.as_deref() == Some("error")
                    && let Ok(custom_error) = CustomError::from_abi(item.clone())
                {
                    contract.add_custom_error(custom_error);
                }
            }

            evm
        };

        // This is an abstract contract
        if contract_evm_output.bytecode.object().is_empty() {
            continue;
        }

        let deployment_bytecode = decode_evm_bytecode(
            contract_rc.clone(),
            solc_version.clone(),
            true,
            &contract_evm_output.bytecode,
            build_model,
        )?;

        let runtime_bytecode = decode_evm_bytecode(
            contract_rc.clone(),
            solc_version.clone(),
            false,
            &contract_evm_output.deployed_bytecode,
            build_model,
        )?;

        bytecodes.push(deployment_bytecode);
        bytecodes.push(runtime_bytecode);
    }

    Ok(bytecodes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        artifacts::{CompilerInput, CompilerOutput, SolcBytecode, SolxBytecode},
        build_model::{solc::SolcBuildModel, solx::SolxBuildModel},
    };

    fn solc_fixture() -> (CompilerInput, CompilerOutput<SolcBytecode>) {
        let input: CompilerInput =
            serde_json::from_str(include_str!("../fixtures/compiler_input.json")).unwrap();
        let output: CompilerOutput<SolcBytecode> =
            serde_json::from_str(include_str!("../fixtures/compiler_output.json")).unwrap();
        (input, output)
    }

    fn solx_fixture() -> (CompilerInput, CompilerOutput<SolxBytecode>) {
        let mut input: CompilerInput =
            serde_json::from_str(include_str!("../fixtures/solx_compiler_input.json")).unwrap();
        input.sources.get_mut("Counter.sol").unwrap().content =
            include_str!("../fixtures/sources/Counter.sol").to_string();
        let output: CompilerOutput<SolxBytecode> =
            serde_json::from_str(include_str!("../fixtures/solx_compiler_output.json")).unwrap();
        (input, output)
    }

    #[test]
    fn solc_fixture_decodes() {
        let (input, output) = solc_fixture();
        let build_model =
            SolcBuildModel::new(input, &output).expect("solc fixture should create a build model");

        let result = populate_decoded_bytecodes("0.8.0".to_string(), build_model, &output);
        assert!(
            result.is_ok(),
            "solc fixture should still decode: {:?}",
            result.err()
        );
    }

    #[test]
    fn solx_fixture_decodes_via_dwarf() {
        let (input, output) = solx_fixture();
        let build_model =
            SolxBuildModel::new(input, &output).expect("solx fixture should create a build model");

        let bytecodes = populate_decoded_bytecodes("0.8.34".to_string(), build_model, &output)
            .expect("solx fixture must decode through the DWARF parser");

        // Creation + runtime.
        assert!(
            bytecodes.len() >= 2,
            "expected at least 2 ContractMetadata for the Counter contract, got {}",
            bytecodes.len()
        );
        // PC → line assertions live in `crate::debug_info::dwarf::tests`.
    }
}
