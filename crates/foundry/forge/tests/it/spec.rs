//! Integration tests for EVM specifications.

use edr_test_utils::SolidityTestFilter;
use foundry_evm::revm::primitives::SpecId;

use crate::{config::*, test_helpers::TEST_DATA_DEFAULT};

#[tokio::test(flavor = "multi_thread")]
async fn test_shanghai_compat() {
    let filter = SolidityTestFilter::new("", "ShanghaiCompat", ".*spec");
    TestConfig::with_filter(TEST_DATA_DEFAULT.runner(), filter)
        .evm_spec(SpecId::SHANGHAI)
        .run()
        .await;
}
