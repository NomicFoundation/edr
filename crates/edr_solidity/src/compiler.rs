//! Processes the Solidity compiler standard JSON[^1] input and output AST and
//! creates the source model used to perform the stack trace decoding.
//!
//! [^1]: See <https://docs.soliditylang.org/en/latest/using-the-compiler.html#compiler-input-and-output-json-description>.
use std::sync::Arc;

use anyhow::{self, Context as _};
use edr_primitives::hex;
use parking_lot::RwLock;

use crate::{
    artifacts::{CompilerArtifact, CompilerOutput, CompilerOutputContract},
    build_model::{BuildModel, Contract, ContractMetadata, CustomError, SourceFile},
    library_utils::{get_library_address_positions, normalize_compiler_output_bytecode},
};

/// First Solc version supported for stack trace generation
pub const FIRST_SOLC_VERSION_SUPPORTED: semver::Version = semver::Version::new(0, 5, 1);

pub(crate) fn correct_selectors<ArtifactT: CompilerArtifact>(
    contracts: &[Arc<ContractMetadata>],
    compiler_output: &CompilerOutput<ArtifactT>,
) -> anyhow::Result<()> {
    for identified in contracts.iter().filter(|c| !c.is_deployment) {
        let mut contract = identified.contract.write();
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
    sources: Arc<[Arc<RwLock<SourceFile>>]>,
) -> anyhow::Result<Arc<ContractMetadata>> {
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

    Ok(Arc::new(ContractMetadata::new(
        sources,
        contract,
        is_deployment,
        normalized_code,
        instructions,
        library_address_positions,
        immutable_references,
        solc_version,
    )))
}

pub(crate) fn decode_bytecodes<BuildModelT: BuildModel>(
    solc_version: String,
    compiler_output: &CompilerOutput<<BuildModelT as BuildModel>::Artifact>,
    build_model: BuildModelT,
    sources: &Arc<[Arc<RwLock<SourceFile>>]>,
) -> anyhow::Result<Vec<Arc<ContractMetadata>>> {
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
            &build_model,
            sources.clone(),
        )?;

        let runtime_bytecode = decode_evm_bytecode(
            contract_rc.clone(),
            solc_version.clone(),
            false,
            &contract_evm_output.deployed_bytecode,
            &build_model,
            sources.clone(),
        )?;

        bytecodes.push(deployment_bytecode);
        bytecodes.push(runtime_bytecode);
    }

    Ok(bytecodes)
}
