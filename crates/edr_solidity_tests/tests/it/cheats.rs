//! Forge tests for cheatcodes.
use foundry_cheatcodes::{FsPermissions, PathPermission};

use crate::helpers::{
    L1ForgeTestData, SolidityTestFilter, TestConfig, RE_PATH_SEPARATOR, TEST_DATA_CANCUN,
    TEST_DATA_DEFAULT, TEST_DATA_MULTI_VERSION,
};

/// Executes all cheat code tests but not fork cheat codes or tests that require
/// isolation mode
async fn test_cheats_local(test_data: &L1ForgeTestData, should_fail: bool) {
    let path_pattern = format!(".*cheats{RE_PATH_SEPARATOR}*");
    let exclude_paths = "Fork";
    let exclude_contracts = "Isolated|Sleep";
    let should_fail_pattern = "testShouldFail*";
    let mut filter = if should_fail {
        SolidityTestFilter::new(should_fail_pattern, ".*", &path_pattern)
            .exclude_paths(exclude_paths)
            .exclude_contracts(exclude_contracts)
    } else {
        SolidityTestFilter::new(".*", ".*", &path_pattern)
            .exclude_paths(exclude_paths)
            .exclude_contracts(exclude_contracts)
            .exclude_tests(should_fail_pattern)
    };


    // Exclude FFI tests on Windows because no `echo`, and file tests that expect
    // certain file paths
    if cfg!(windows) {
        filter = filter.exclude_tests("(Ffi|File|Line|Root)");
    }

    let runner = test_data
        .runner_with_fs_permissions(
            FsPermissions::new(vec![PathPermission::read_write_directory("./fixtures")]),
            test_data.config_with_mock_rpc(),
        )
        .await;

    TestConfig::with_filter(runner, filter).set_should_fail(should_fail).run().await;
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
async fn test_cheats_local_multi_version() {
    test_cheats_local(&TEST_DATA_MULTI_VERSION, false).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn test_cheats_local_multi_version_should_fail() {
    test_cheats_local(&TEST_DATA_MULTI_VERSION, true).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn test_cheats_local_cancun() {
    test_cheats_local(&TEST_DATA_CANCUN, false).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn test_cheats_local_cancun_should_fail() {
    test_cheats_local(&TEST_DATA_CANCUN, true).await;
}
