use std::collections::BTreeMap;

use edr_solidity::{artifacts::BuildInfo, contract_decoder::ContractDecoder};
use edr_solidity_tests::{
    contracts::{ArtifactId, ContractData, ContractsByArtifact},
    decode::RevertDecoder,
    multi_runner::{TestContract, TestContracts},
    MultiContractRunner, SolidityTestRunnerConfig,
};
use napi::bindgen_prelude::Uint8Array;

use crate::solidity_tests::{
    artifact::{Artifact as JsArtifact, ArtifactId as JsArtifactId},
    config::SolidityTestRunnerConfigArgs,
};

pub(super) async fn build_runner(
    artifacts: Vec<JsArtifact>,
    test_suites: Vec<JsArtifactId>,
    config_args: SolidityTestRunnerConfigArgs,
    build_infos: Vec<Uint8Array>,
) -> napi::Result<MultiContractRunner> {
    let known_contracts: ContractsByArtifact = artifacts
        .into_iter()
        .map(|item| Ok((item.id.try_into()?, item.contract.try_into()?)))
        .collect::<Result<BTreeMap<ArtifactId, ContractData>, napi::Error>>()?
        .into();

    let test_suites = test_suites
        .into_iter()
        .map(TryInto::try_into)
        .collect::<Result<Vec<ArtifactId>, _>>()?;

    let config: SolidityTestRunnerConfig = config_args.try_into()?;

    // TODO parallelize
    // TODO if (semver.gte(buildInfo.solcVersion, FIRST_SOLC_VERSION_SUPPORTED)) {
    let build_infos = build_infos
        .into_iter()
        .map(|build_info| serde_json::from_slice(build_info.as_ref()))
        .collect::<Result<Vec<BuildInfo>, _>>()?;
    let build_info_config = edr_solidity::contract_decoder::BuildInfoConfig {
        build_infos: Some(build_infos),
        ignore_contracts: None,
    };
    let contract_decoder = ContractDecoder::new(&build_info_config)
        .map_err(|error| napi::Error::from_reason(error.to_string()))?;

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

    MultiContractRunner::new(
        config,
        contracts,
        known_contracts,
        contract_decoder,
        revert_decoder,
    )
    .await
    .map_err(|err| {
        napi::Error::new(
            napi::Status::GenericFailure,
            format!("Failed to create multi contract runner: {err}"),
        )
    })
}
