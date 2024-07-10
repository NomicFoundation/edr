use std::sync::Arc;

/// Based on `crates/foundry/forge/tests/it/test_helpers.rs`.
use forge::{
    decode::RevertDecoder, multi_runner::TestContract, revm::primitives::SpecId,
    MultiContractRunner, TestOptionsBuilder,
};
use foundry_compilers::ArtifactId;

use crate::solidity_tests::config::SolidityTestsConfig;

pub(super) fn build_runner(
    test_suites: Vec<(ArtifactId, TestContract)>,
    gas_report: bool,
) -> napi::Result<MultiContractRunner> {
    let config = SolidityTestsConfig::new(gas_report);

    let SolidityTestsConfig {
        evm_opts,
        project_paths_config,
        cheats_config_options,
        fuzz,
        invariant,
    } = config;

    let test_options = TestOptionsBuilder::default()
        .fuzz(fuzz)
        .invariant(invariant)
        .build_hardhat()
        .map_err(|e| napi::Error::new(napi::Status::GenericFailure, format!("{e:?}")))?;

    let abis = test_suites.iter().map(|(_, contract)| &contract.abi);
    let revert_decoder = RevertDecoder::new().with_abis(abis);

    let sender = Some(evm_opts.sender);
    let evm_env = evm_opts.local_evm_env();

    Ok(MultiContractRunner {
        project_paths_config: Arc::new(project_paths_config),
        cheats_config_opts: Arc::new(cheats_config_options),
        contracts: test_suites.into_iter().collect(),
        evm_opts,
        env: evm_env,
        evm_spec: SpecId::MERGE,
        sender,
        revert_decoder,
        fork: None,
        coverage: false,
        debug: false,
        test_options,
        isolation: false,
        output: None,
    })
}
