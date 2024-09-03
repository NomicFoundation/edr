use forge::{
    decode::RevertDecoder,
    multi_runner::{TestContract, TestContracts},
    MultiContractRunner, SolidityTestRunnerConfig,
};
use foundry_common::ContractsByArtifact;

use crate::solidity_tests::config::SolidityTestRunnerConfigArgs;

pub(super) async fn build_runner(
    known_contracts: &ContractsByArtifact,
    test_suites: Vec<foundry_common::ArtifactId>,
    config_args: SolidityTestRunnerConfigArgs,
) -> napi::Result<MultiContractRunner> {
    let config: SolidityTestRunnerConfig = config_args.try_into()?;

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
        .collect::<napi::Result<TestContracts>>()?;

    MultiContractRunner::new(config, contracts, known_contracts.clone(), revert_decoder)
        .await
        .map_err(|err| {
            napi::Error::new(
                napi::Status::GenericFailure,
                format!("Failed to create multi contract runner: {err}"),
            )
        })
}
