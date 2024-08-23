use std::sync::Arc;

use forge::{
    decode::RevertDecoder,
    multi_runner::{DeployableContracts, TestContract},
    revm::primitives::SpecId,
    MultiContractRunner, TestOptions,
};
use foundry_common::ContractsByArtifact;

use crate::solidity_tests::config::{SolidityTestRunnerConfig, SolidityTestRunnerConfigArgs};

pub(super) async fn build_runner(
    known_contracts: &ContractsByArtifact,
    test_suites: Vec<foundry_common::ArtifactId>,
    config_args: SolidityTestRunnerConfigArgs,
) -> napi::Result<MultiContractRunner> {
    let config: SolidityTestRunnerConfig = config_args.try_into()?;

    let fork = config.get_fork().await?;

    let SolidityTestRunnerConfig {
        debug,
        trace,
        evm_opts,
        project_root,
        cheats_config_options,
        fuzz,
        invariant,
    } = config;

    let test_options = TestOptions { fuzz, invariant };

    // Build revert decoder from ABIs of all artifacts.
    let abis = known_contracts.iter().map(|(_, contract)| &contract.abi);
    let revert_decoder = RevertDecoder::new().with_abis(abis);

    let contracts = test_suites
        .iter()
        .map(|artifact_id| {
            let contract_data = known_contracts.get(artifact_id).ok_or_else(|| {
                napi::Error::new(
                    napi::Status::GenericFailure,
                    format!("Unknown contract: {}", artifact_id.identifier()),
                )
            })?;

            let bytecode = contract_data.bytecode.clone().ok_or_else(|| {
                napi::Error::new(
                    napi::Status::GenericFailure,
                    format!(
                        "No bytecode for test suite contract: {}",
                        artifact_id.identifier()
                    ),
                )
            })?;

            let test_contract = TestContract::new_hardhat(contract_data.abi.clone(), bytecode);

            Ok((artifact_id.clone(), test_contract))
        })
        .collect::<napi::Result<DeployableContracts>>()?;

    let sender = Some(evm_opts.sender);
    let isolate = evm_opts.isolate;
    let evm_env = evm_opts.local_evm_env();

    Ok(MultiContractRunner {
        project_root,
        cheats_config_opts: Arc::new(cheats_config_options),
        contracts,
        evm_opts,
        env: evm_env,
        evm_spec: SpecId::CANCUN,
        sender,
        revert_decoder,
        fork,
        coverage: false,
        trace,
        debug,
        test_options,
        isolation: isolate,
        output: None,
    })
}
