//! Integration tests for EVM specifications.

use edr_solidity_tests::revm::primitives::hardfork::SpecId;

use crate::helpers::{SolidityTestFilter, TestConfig, TEST_DATA_DEFAULT};

#[tokio::test(flavor = "multi_thread")]
async fn test_shanghai_compat() {
    let filter = SolidityTestFilter::new("", "ShanghaiCompat", ".*spec");
    let mut config = TEST_DATA_DEFAULT.config_with_mock_rpc();
    config.evm_opts.spec = SpecId::SHANGHAI;
    TestConfig::with_filter(TEST_DATA_DEFAULT.runner_with_config(config).await, filter)
        .run()
        .await;
}
