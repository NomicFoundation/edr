//! Integration tests for EVM specifications.

use std::collections::BTreeMap;

use edr_solidity_tests::{result::TestStatus, revm::primitives::hardfork::SpecId};

use crate::helpers::{
    assert_multiple, ForgeTestProfile, L1ForgeTestData, SolidityTestFilter, TestConfig,
    TEST_DATA_PARIS,
};

#[tokio::test(flavor = "multi_thread")]
async fn test_shanghai_compat() {
    let filter = SolidityTestFilter::new("", "ShanghaiCompat", ".*spec");
    let mut config = TEST_DATA_PARIS.config_with_mock_rpc();
    config.evm_opts.spec = SpecId::SHANGHAI;
    TestConfig::with_filter(
        TEST_DATA_PARIS.runner_with_fuzz_persistence(config).await,
        filter,
    )
    .run()
    .await;
}

#[tokio::test(flavor = "multi_thread")]
async fn test_function_override_evm_version() {
    let filter = SolidityTestFilter::new(".*", ".*", ".*spec/ShanghaiCompat.t.sol");

    // Without override, PUSH0 is not available in the Merge spec, so the test
    // fails.
    let config = TEST_DATA_PARIS.config_with_mock_rpc();
    let runner = TEST_DATA_PARIS.runner_with_fuzz_persistence(config).await;
    let results = runner
        .test_collect(filter.clone())
        .await
        .expect("the run produces results")
        .suite_results;

    assert_multiple(
        &results,
        BTreeMap::from([(
            "paris/spec/ShanghaiCompat.t.sol:ShanghaiCompat",
            vec![("testPush0()", false, None, None, None)],
        )]),
    );

    // With the inline `evmVersion` directive (in `ShanghaiCompatOverride.t.sol`)
    // set to Shanghai, PUSH0 becomes available and the test passes.
    let override_filter =
        SolidityTestFilter::new(".*", ".*", ".*spec/ShanghaiCompatOverride.t.sol");
    let config = TEST_DATA_PARIS.config_with_mock_rpc();
    let runner = TEST_DATA_PARIS.runner_with_fuzz_persistence(config).await;
    let results = runner
        .test_collect(override_filter)
        .await
        .expect("the run produces results")
        .suite_results;

    assert_multiple(
        &results,
        BTreeMap::from([(
            "paris/spec/ShanghaiCompatOverride.t.sol:ShanghaiCompatOverride",
            vec![("testPush0()", true, None, None, None)],
        )]),
    );
}

/// A test that lowers the hardfork below Amsterdam with an inline
/// `evm_version` must run like it does without the override.
#[tokio::test(flavor = "multi_thread")]
async fn test_function_override_evm_version_below_amsterdam() {
    let test_data =
        L1ForgeTestData::new(ForgeTestProfile::Default, edr_chain_l1::Hardfork::Amsterdam)
            .expect("linking ok");
    let filter = SolidityTestFilter::new(".*", ".*", ".*spec/EvmVersionBelowAmsterdam.t.sol");
    let runner = test_data.runner().await;
    let results = runner
        .test_collect(filter)
        .await
        .expect("the run produces results")
        .suite_results;

    assert_multiple(
        &results,
        BTreeMap::from([
            (
                "default/spec/EvmVersionBelowAmsterdam.t.sol:EvmVersionAmsterdamTest",
                vec![("testCall()", true, None, None, None)],
            ),
            (
                "default/spec/EvmVersionBelowAmsterdam.t.sol:EvmVersionOsakaTest",
                vec![("testCall()", true, None, None, None)],
            ),
        ]),
    );
}

/// An inline `evm_version` override only derives the gas limit again for the
/// overridden hardfork if it isn't configured explicitly: a configured gas
/// limit or block gas limit above the EIP-7825 cap is still used as is.
#[tokio::test(flavor = "multi_thread")]
async fn test_function_override_evm_version_below_amsterdam_keeps_configured_gas_limit() {
    // Above the EIP-7825 cap of 2^24.
    const GAS_LIMIT: u64 = 30_000_000;

    let test_data =
        L1ForgeTestData::new(ForgeTestProfile::Default, edr_chain_l1::Hardfork::Amsterdam)
            .expect("linking ok");
    let filter = SolidityTestFilter::new(".*", ".*", ".*spec/EvmVersionBelowAmsterdam.t.sol");

    let mut gas_limit_config = test_data.config_with_mock_rpc();
    gas_limit_config.evm_opts.env.gas_limit = Some(GAS_LIMIT);
    let mut block_gas_limit_config = test_data.config_with_mock_rpc();
    block_gas_limit_config.evm_opts.env.block_gas_limit = Some(GAS_LIMIT);

    for config in [gas_limit_config, block_gas_limit_config] {
        let runner = test_data.runner_with_fuzz_persistence(config).await;
        let results = runner
            .test_collect(filter.clone())
            .await
            .expect("the run produces results")
            .suite_results;

        let amsterdam_result = &results
            ["default/spec/EvmVersionBelowAmsterdam.t.sol:EvmVersionAmsterdamTest"]
            .test_results["testCall()"];
        assert_eq!(amsterdam_result.status, TestStatus::Success);

        let osaka_result = &results
            ["default/spec/EvmVersionBelowAmsterdam.t.sol:EvmVersionOsakaTest"]
            .test_results["testCall()"];
        assert_eq!(osaka_result.status, TestStatus::Failure);
        let reason = osaka_result.reason.as_deref().unwrap_or_default();
        assert!(
            reason.contains(&format!(
                "transaction gas limit ({GAS_LIMIT}) is greater than the cap"
            )),
            "unexpected failure reason: {reason}"
        );
    }
}
