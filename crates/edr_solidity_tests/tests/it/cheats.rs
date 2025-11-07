//! Forge tests for cheatcodes.
use alloy_primitives::U256;
use edr_solidity_tests::result::{TestKind, TestStatus};
use foundry_cheatcodes::{FsPermissions, PathPermission};

use crate::helpers::{
    L1ForgeTestData, SolidityTestFilter, TestConfig, RE_PATH_SEPARATOR, TEST_DATA_DEFAULT,
    TEST_DATA_MULTI_VERSION, TEST_DATA_PARIS,
};

/// Executes all cheat code tests but not fork cheat codes or tests that require
/// isolation mode
async fn test_cheats_local(test_data: &L1ForgeTestData, should_fail: bool) {
    let path_pattern = format!(".*cheats{RE_PATH_SEPARATOR}*");
    let exclude_paths = "Fork";
    let exclude_contracts = "Isolated|Sleep|WithSeed";
    let should_fail_pattern = "testShouldFail";
    let windows_exclude_patterns = ["Ffi", "File", "Line", "Root"];

    let filter = if should_fail {
        SolidityTestFilter::new(should_fail_pattern, ".*", &path_pattern)
            .exclude_paths(exclude_paths)
            .exclude_contracts(exclude_contracts)
    } else {
        SolidityTestFilter::new(".*", ".*", &path_pattern)
            .exclude_paths(exclude_paths)
            .exclude_contracts(exclude_contracts)
    };

    let mut exclude_test_patterns = Vec::default();

    if !should_fail {
        exclude_test_patterns.push(should_fail_pattern);
    }

    // Exclude FFI tests on Windows because no `echo`, and file tests that expect
    // certain file paths
    if cfg!(windows) {
        exclude_test_patterns.extend_from_slice(&windows_exclude_patterns);
    }

    let filter = filter.exclude_tests(&format!("({})", exclude_test_patterns.join("|")));

    let runner = test_data
        .runner_with_fs_permissions(
            FsPermissions::new(vec![PathPermission::read_write_directory("./fixtures")]),
            test_data.config_with_remote_rpc(),
        )
        .await;

    TestConfig::with_filter(runner, filter)
        .set_should_fail(should_fail)
        .run()
        .await;
}

/// Executes subset of all cheat code tests in isolation mode
async fn test_cheats_local_isolated(test_data: &L1ForgeTestData) {
    let filter = SolidityTestFilter::new(
        ".*",
        ".*(Isolated)",
        &format!(".*cheats{RE_PATH_SEPARATOR}*"),
    );

    let mut config = test_data.config_with_mock_rpc();
    config.evm_opts.isolate = true;
    let runner = test_data.runner_with_config(config).await;

    TestConfig::with_filter(runner, filter).run().await;
}

/// Executes subset of all cheat code tests using a specific seed.
async fn test_cheats_local_with_seed(test_data: &L1ForgeTestData) {
    let filter = SolidityTestFilter::new(
        ".*",
        ".*(WithSeed)",
        &format!(".*cheats{RE_PATH_SEPARATOR}*"),
    );

    let mut config = test_data.config_with_mock_rpc();
    config.cheats_config_options.seed = Some(U256::from(100));
    let runner = test_data.runner_with_config(config).await;

    TestConfig::with_filter(runner, filter).run().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn test_cheats_local_default() {
    test_cheats_local(&TEST_DATA_DEFAULT, false).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn test_cheats_local_should_fail() {
    test_cheats_local(&TEST_DATA_DEFAULT, true).await;
}

// Need custom fuzz config to speed it up
#[tokio::test(flavor = "multi_thread")]
async fn test_cheats_sleep_test() {
    let filter = SolidityTestFilter::new(".*", "Sleep", &format!(".*cheats{RE_PATH_SEPARATOR}*"));

    let mut runner_config = TEST_DATA_DEFAULT.config_with_mock_rpc();
    runner_config.fuzz.runs = 2;
    let runner = TEST_DATA_DEFAULT.runner_with_config(runner_config).await;

    TestConfig::with_filter(runner, filter).run().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn test_cheats_local_default_isolated() {
    test_cheats_local_isolated(&TEST_DATA_DEFAULT).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn test_cheats_local_default_with_seed() {
    test_cheats_local_with_seed(&TEST_DATA_DEFAULT).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn test_cheats_local_multi_version() {
    test_cheats_local(&TEST_DATA_MULTI_VERSION, false).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn test_cheats_local_multi_version_should_fail() {
    test_cheats_local(&TEST_DATA_MULTI_VERSION, true).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn test_cheats_local_paris() {
    test_cheats_local(&TEST_DATA_PARIS, false).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn test_cheats_local_paris_should_fail() {
    test_cheats_local(&TEST_DATA_PARIS, true).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn test_gas_metering_reset() {
    let filter = SolidityTestFilter::new(".*", "GasMeteringResetTest", ".*cheats/");
    let config = TEST_DATA_DEFAULT.config_with_mock_rpc();
    let runner = TEST_DATA_DEFAULT.runner_with_config(config).await;
    let suite_results = runner.test_collect(filter).await.suite_results;

    let suite_result = suite_results
        .get("default/cheats/GasMeteringReset.t.sol:GasMeteringResetTest")
        .unwrap();

    // None indicates that we don't match the exact value
    let expected_gas = [
        ("testResetGas()", Some(96)),
        ("testResetGas1()", Some(96)),
        ("testResetGas2()", Some(96)),
        ("testResetGas3()", None),
        ("testResetGas4()", None),
        ("testResetGas5()", Some(96)),
        ("testResetGas6()", Some(96)),
        ("testResetGas7()", Some(96)),
        ("testResetGas8()", None),
        ("testResetGas9()", Some(96)),
        ("testResetNegativeGas()", Some(96)),
    ];

    for (test_name, gas) in expected_gas {
        let test_result = suite_result.test_results.get(test_name).unwrap();
        assert_eq!(test_result.status, TestStatus::Success);
        match gas {
            Some(gas) => assert!(matches!(test_result.kind, TestKind::Unit { gas: g } if g == gas)),
            None => assert!(matches!(test_result.kind, TestKind::Unit { gas: _ })),
        }
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn test_expect_partial_revert() {
    let filter = SolidityTestFilter::new(".*", "ExpectPartialRevertTest", ".*cheats/");
    let config = TEST_DATA_DEFAULT.config_with_mock_rpc();
    let runner = TEST_DATA_DEFAULT.runner_with_config(config).await;
    let suite_results = runner.test_collect(filter).await.suite_results;

    let suite_result = suite_results
        .get("default/cheats/ExpectPartialRevert.t.sol:ExpectPartialRevertTest")
        .unwrap();

    let test_result = suite_result.test_results.get("testExpectPartialRevertWithSelector()").unwrap();
    assert_eq!(test_result.status, TestStatus::Success);

    let test_result = suite_result.test_results.get("testExpectPartialRevertWith4Bytes()").unwrap();
    assert_eq!(test_result.status, TestStatus::Success);

    let test_result = suite_result.test_results.get("testExpectRevert()").unwrap();
    assert_eq!(test_result.status, TestStatus::Failure);
    assert_eq!(test_result.reason, Some("Error != expected error: WrongNumber(0) != custom error 0x238ace70".into()));
}

#[tokio::test(flavor = "multi_thread")]
async fn test_assume_no_revert() {
    let filter = SolidityTestFilter::new(".*", "AssumeNoRevertTest", ".*cheats/");
    let mut config = TEST_DATA_DEFAULT.config_with_mock_rpc();
    config.fuzz.runs = 100;
    config.fuzz.seed = Some(U256::from(100));
    let runner = TEST_DATA_DEFAULT.runner_with_config(config).await;
    let suite_results = runner.test_collect(filter).await.suite_results;

    let suite_result = suite_results
        .get("default/cheats/AssumeNoRevert2.t.sol:AssumeNoRevertTest")
        .unwrap();

    let test_result = suite_result.test_results.get("test_assume_no_revert_pass(uint256)").unwrap();
    assert_eq!(test_result.status, TestStatus::Success);

    let test_result = suite_result.test_results.get("test_assume_no_revert_fail_assert(uint256)").unwrap();
    assert_eq!(test_result.status, TestStatus::Failure);
    assert!(test_result.counterexample.is_some());

    let test_result = suite_result.test_results.get("test_assume_no_revert_fail_in_2nd_call(uint256)").unwrap();
    assert_eq!(test_result.status, TestStatus::Failure);
    assert_eq!(test_result.reason, Some("CheckError()".into()));
    assert!(test_result.counterexample.is_some());

    let test_result = suite_result.test_results.get("test_assume_no_revert_fail_in_3rd_call(uint256)").unwrap();
    assert_eq!(test_result.status, TestStatus::Failure);
    assert_eq!(test_result.reason, Some("CheckError()".into()));
    assert!(test_result.counterexample.is_some());
}

#[tokio::test(flavor = "multi_thread")]
async fn test_assume_no_revert_with_data() {
    let filter = SolidityTestFilter::new(".*", "AssumeNoRevertWithDataTest", ".*cheats/");
    let mut config = TEST_DATA_DEFAULT.config_with_mock_rpc();
    config.fuzz.seed = Some(U256::from(100));
    let runner = TEST_DATA_DEFAULT.runner_with_config(config).await;
    let suite_results = runner.test_collect(filter).await.suite_results;

    let suite_result = suite_results
        .get("default/cheats/AssumeNoRevertWithData.t.sol:AssumeNoRevertWithDataTest")
        .unwrap();

    let test_result = suite_result.test_results.get("testAssumeThenExpectCountZeroFails(uint256)").unwrap();
    assert_eq!(test_result.status, TestStatus::Failure);
    assert_eq!(test_result.reason, Some("call reverted with 'FOUNDRY::ASSUME' when it was expected not to revert".into()));
    assert!(test_result.counterexample.is_some());

    let test_result = suite_result.test_results.get("testAssumeWithReverter_fails(uint256)").unwrap();
    assert_eq!(test_result.status, TestStatus::Failure);
    assert_eq!(test_result.reason, Some("MyRevert()".into()));
    assert!(test_result.counterexample.is_some());

    let test_result = suite_result.test_results.get("testAssume_wrongData_fails(uint256)").unwrap();
    assert_eq!(test_result.status, TestStatus::Failure);
    assert_eq!(test_result.reason, Some("RevertWithData(2)".into()));
    assert!(test_result.counterexample.is_some());

    let test_result = suite_result.test_results.get("testAssume_wrongSelector_fails(uint256)").unwrap();
    assert_eq!(test_result.status, TestStatus::Failure);
    assert_eq!(test_result.reason, Some("MyRevert()".into()));
    assert!(test_result.counterexample.is_some());

    let test_result = suite_result.test_results.get("testExpectCountZeroThenAssumeFails(uint256)").unwrap();
    assert_eq!(test_result.status, TestStatus::Failure);
    assert_eq!(test_result.reason, Some("call reverted with 'FOUNDRY::ASSUME' when it was expected not to revert".into()));
    assert!(test_result.counterexample.is_some());

    let test_result = suite_result.test_results.get("testMultipleAssumesClearAfterCall_fails(uint256)").unwrap();
    assert_eq!(test_result.status, TestStatus::Failure);
    assert_eq!(test_result.reason, Some("MyRevert()".into()));
    assert!(test_result.counterexample.is_some());

    let test_result = suite_result.test_results.get("testMultipleAssumes_OneWrong_fails(uint256)").unwrap();
    assert_eq!(test_result.status, TestStatus::Failure);
    assert_eq!(test_result.reason, Some("RevertWithData(3)".into()));
    assert!(test_result.counterexample.is_some());

    let test_result = suite_result.test_results.get("testMultipleAssumes_ThrowOnGenericNoRevert_AfterSpecific_fails(bytes4)").unwrap();
    assert_eq!(test_result.status, TestStatus::Failure);
    assert_eq!(test_result.reason, Some("vm.assumeNoRevert: you must make another external call prior to calling assumeNoRevert again".into()));
    assert!(test_result.counterexample.is_some());
}
