//! Integration tests for EVM specifications.

use std::collections::BTreeMap;

use edr_solidity_tests::{
    executors::stack_trace::SolidityTestStackTraceResult, result::TestStatus,
    revm::primitives::hardfork::SpecId, CollectStackTraces,
};

use crate::helpers::{
    assert_multiple, contract_decoder, SolidityTestFilter, TestConfig, TEST_DATA_PARIS,
    TEST_DATA_VIA_IR,
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

    // With the inline `evmVersion` directive (in `ShanghaiCompatOverride.t.sol`)
    // set to Shanghai, PUSH0 becomes available and the test passes.
    let override_filter =
        SolidityTestFilter::new(".*", ".*", ".*spec/ShanghaiCompatOverride.t.sol");
    let config = TEST_DATA_PARIS.config_with_mock_rpc();
    let runner = TEST_DATA_PARIS.runner_with_fuzz_persistence(config).await;
    let results = runner.test_collect(override_filter).await.suite_results;

    assert_multiple(
        &results,
        BTreeMap::from([(
            "paris/spec/ShanghaiCompatOverride.t.sol:ShanghaiCompatOverride",
            vec![("testPush0()", true, None, None, None)],
        )]),
    );
}

// The re-run that computes a failing test's stack trace must apply the test's
// executor overrides after `setUp()`, as the original run does: here the suite
// runs at a hardfork with PUSH0, `setUp()` needs it, and the test's inline
// `evmVersion` directive (in `OverrideAfterSetup.t.sol`) selects Merge (Paris),
// which predates PUSH0.
#[tokio::test(flavor = "multi_thread")]
async fn test_stack_trace_re_run_applies_overrides_after_setup() {
    let filter = SolidityTestFilter::new(".*", ".*", ".*spec/OverrideAfterSetup.t.sol");
    let mut config = TEST_DATA_VIA_IR.config_with_mock_rpc();
    // Only this mode re-executes the failing test for its stack trace; with
    // `Always` the trace would come from the original run's recorded steps.
    config.collect_stack_traces = CollectStackTraces::OnFailure;

    // A real decoder, so the re-run's trace is decoded against the test
    // contract's sources.
    let contract_decoder = contract_decoder(TEST_DATA_VIA_IR.build_info_path());
    let runner = TEST_DATA_VIA_IR
        .runner_with_contract_decoder(config, contract_decoder)
        .await;
    let results = runner.test_collect(filter).await.suite_results;
    let suite = results
        .get("via-ir/spec/OverrideAfterSetup.t.sol:OverrideAfterSetup")
        .expect("the OverrideAfterSetup suite should have run");

    // Unit and fuzz tests are re-run by different functions; both must apply
    // the override after setup.
    for test_name in ["testRevertsAtMerge()", "testFuzzRevertsAtMerge(uint256)"] {
        let result = suite
            .test_results
            .get(test_name)
            .unwrap_or_else(|| panic!("{test_name} should have run"));

        assert_eq!(result.status, TestStatus::Failure, "{test_name}");
        assert!(
            matches!(
                result.stack_trace_result,
                Some(SolidityTestStackTraceResult::Success(_))
            ),
            "{test_name}: expected a stack trace, got {:?} (reason: {:?})",
            result.stack_trace_result,
            result.reason
        );
    }
}
