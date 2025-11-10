//! Integration tests for EVM specifications.

use edr_solidity_tests::revm::primitives::hardfork::SpecId;

use crate::helpers::{SolidityTestFilter, TestConfig, TEST_DATA_PARIS};

#[tokio::test(flavor = "multi_thread")]
async fn test_shanghai_compat() {
    let filter = SolidityTestFilter::new("", "ShanghaiCompat", ".*spec");
    let mut config = TEST_DATA_PARIS.config_with_mock_rpc();
    config.evm_opts.spec = SpecId::SHANGHAI;
    TestConfig::with_filter(TEST_DATA_PARIS.runner_with_fuzz_persistence(config).await, filter)
        .run()
        .await;
}
