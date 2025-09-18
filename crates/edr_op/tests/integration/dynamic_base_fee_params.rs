#![cfg(feature = "test-remote")]

use std::sync::LazyLock;

use edr_block_header::{BlockHeader, HeaderOverrides};
use edr_eip1559::{BaseFeeActivation, BaseFeeParams, ConstantBaseFeeParams, DynamicBaseFeeParams};
use edr_evm::impl_full_block_tests;
use edr_op::{Hardfork, OpChainSpec};
use edr_provider::test_utils::header_overrides;

use super::op::mainnet_url;

static OP_BASE_FEE_PARAMS: LazyLock<BaseFeeParams<Hardfork>> = LazyLock::new(|| {
    BaseFeeParams::Dynamic(DynamicBaseFeeParams::new(vec![
        (
            BaseFeeActivation::Hardfork(Hardfork::BEDROCK),
            ConstantBaseFeeParams::new(50, 6),
        ),
        (
            BaseFeeActivation::Hardfork(Hardfork::CANYON),
            ConstantBaseFeeParams::new(250, 6),
        ),
        // On OP mainnet, the first block to have extra_data field with (250, 4) is 135_513_415
        // but we are configuring 135_513_416 here since it is the first block to which those
        // params must be applied for the base_fee calculation
        (
            BaseFeeActivation::BlockNumber(135_513_416),
            ConstantBaseFeeParams::new(250, 4),
        ), // ConfigUpdate timestamp: 0x681b63b7, logIndex: 0x1e8
        (
            BaseFeeActivation::BlockNumber(136_165_876),
            ConstantBaseFeeParams::new(250, 2),
        ), // ConfigUpdate timestamp: 0x682f4d53, logIndex: 0x1ed
    ]))
});

fn op_header_overrides(replay_header: &BlockHeader) -> HeaderOverrides<Hardfork> {
    HeaderOverrides {
        base_fee_params: Some(OP_BASE_FEE_PARAMS.clone()),
        ..header_overrides(replay_header)
    }
}
impl_full_block_tests! {
    // Validate that overriding base fee params
    // EDR behaves the same way as the chain:
    //  - First updates the previous activation block extra data field
    //  - On the following block - on the activation point - uses the new base fee params
    // We can validate this by comparing with the replay block without overriding the `extra_data` field
    mainnet_system_config_update_on_extra_data => OpChainSpec {
        block_number: 135_513_415,
        url: mainnet_url(),
        header_overrides_constructor: op_header_overrides,
    },
    mainnet_after_system_config_update => OpChainSpec {
        block_number: 135_513_416,
        url: mainnet_url(),
        header_overrides_constructor: op_header_overrides,
    },
}
