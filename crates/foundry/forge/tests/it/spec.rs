//! Integration tests for EVM specifications.

use foundry_evm::revm::primitives::SpecId;
use foundry_test_utils::Filter;

use crate::{config::*, test_helpers::TEST_DATA_DEFAULT};

#[tokio::test(flavor = "multi_thread")]
async fn test_shanghai_compat() {
    let filter = Filter::new("", "ShanghaiCompat", ".*spec");
    TestConfig::with_filter(TEST_DATA_DEFAULT.runner(), filter)
        .evm_spec(SpecId::SHANGHAI)
        .run()
        .await;
}
