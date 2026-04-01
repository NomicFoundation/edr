//! Integration tests for EVM specifications.

use std::collections::BTreeMap;

use edr_solidity_tests::revm::primitives::hardfork::SpecId;

use crate::helpers::{
    assert_multiple, make_test_identifier, SolidityTestFilter, TestConfig, TEST_DATA_PARIS,
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
    let results = runner.test_collect(filter.clone()).await.suite_results;

    assert_multiple(
        &results,
        BTreeMap::from([(
            "paris/spec/ShanghaiCompat.t.sol:ShanghaiCompat",
            vec![("testPush0()", false, None, None, None)],
        )]),
    );

    // With the evm_version override to Shanghai, PUSH0 becomes available and the
    // test passes.
    let mut config = TEST_DATA_PARIS.config_with_mock_rpc();
    config.test_function_overrides.insert(
        make_test_identifier(
            "paris/spec/ShanghaiCompat.t.sol:ShanghaiCompat",
            "testPush0()",
        ),
        edr_solidity_tests::TestFunctionConfigOverride {
            allow_internal_expect_revert: None,
            isolate: None,
            evm_version: Some("Shanghai".to_string()),
            fuzz: None,
            invariant: None,
        },
    );

    let runner = TEST_DATA_PARIS.runner_with_fuzz_persistence(config).await;
    let results = runner.test_collect(filter).await.suite_results;

    assert_multiple(
        &results,
        BTreeMap::from([(
            "paris/spec/ShanghaiCompat.t.sol:ShanghaiCompat",
            vec![("testPush0()", true, None, None, None)],
        )]),
    );
}
