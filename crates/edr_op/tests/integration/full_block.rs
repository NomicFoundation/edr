#![cfg(feature = "test-remote")]

use std::sync::LazyLock;

use edr_eth::{
    block::{self, HeaderOverrides},
    eips::eip1559::{
        BaseFeeActivation, BaseFeeParams, ConstantBaseFeeParams, VariableBaseFeeParams,
    },
};
use edr_evm::impl_full_block_tests;
use edr_op::{test_utils::isthmus_header_overrides, OpChainSpec, OpSpecId};
use edr_provider::test_utils::header_overrides;

use super::op::mainnet_url;

static OP_BASE_FEE_PARAMS: LazyLock<BaseFeeParams<OpSpecId>> = LazyLock::new(|| {
    BaseFeeParams::Variable(VariableBaseFeeParams::new(vec![
        (
            BaseFeeActivation::Hardfork(OpSpecId::BEDROCK),
            ConstantBaseFeeParams::new(50, 6),
        ),
        (
            BaseFeeActivation::Hardfork(OpSpecId::CANYON),
            ConstantBaseFeeParams::new(250, 6),
        ),
        // TODO: document that on OP, the base_fee params get activated not on the first block
        // that has the fields in the extra_data, but on the next block, since if Holocene is
        // activated then it uses the fields decoded from `parent_header.extraData`.
        // EDR will consider the activation point as an inclusive range
        // On OP mainnet, the first block to have extra_data field with (250, 4) is 135_513_415
        // but we are configuring 135_513_416 here since ..416 is the first block to which those
        // params must be applied
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

fn op_header_overrides(replay_header: &block::Header) -> HeaderOverrides<OpSpecId> {
    HeaderOverrides {
        base_fee_params: Some(OP_BASE_FEE_PARAMS.clone()),
        ..header_overrides(replay_header)
    }
}
impl_full_block_tests! {
    mainnet_regolith => OpChainSpec {
        block_number: 105_235_064,
        url: mainnet_url(),
        header_overrides_constructor: op_header_overrides,
    },
    mainnet_canyon => OpChainSpec {
        block_number: 115_235_064,
        url: mainnet_url(),
        header_overrides_constructor: op_header_overrides,
    },
    mainnet_ecotone => OpChainSpec {
        block_number: 121_874_088,
        url: mainnet_url(),
        header_overrides_constructor: op_header_overrides,
    },
    mainnet_fjord => OpChainSpec {
        block_number: 122_514_212,
        url: mainnet_url(),
        header_overrides_constructor: op_header_overrides,
    },
    mainnet_granite => OpChainSpec {
        block_number: 125_235_823,
        url: mainnet_url(),
        header_overrides_constructor: op_header_overrides,
    },
    // The first Holocene block used a dynamic base fee set in the SystemConfig.
    mainnet_holocene => OpChainSpec {
        block_number: 130_423_412,
        url: mainnet_url(),
        header_overrides_constructor: op_header_overrides,
    },
    // The second Holocene block should use the dynamic base fee from the parent block's `extra_data`.
    mainnet_holocene_plus_one => OpChainSpec {
        block_number: 130_423_413,
        url: mainnet_url(),
        header_overrides_constructor: op_header_overrides,
    },
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
    // The Isthmus hardfork modified the GasPriceOracle predeploy in this block
    // but we don't support forked account overrides yet.
    // mainnet_isthmus => OpChainSpec {
    //     block_number: 135_603_812,
    //     url: mainnet_url(),
    //     header_overrides_constructor: isthmus_header_overrides,
    // },
    mainnet_isthmus_plus_one => OpChainSpec {
        block_number: 135_603_813,
        url: mainnet_url(),
        header_overrides_constructor: isthmus_header_overrides,
    },
    mainnet_137620147 => OpChainSpec {
        block_number: 137_620_147,
        url: mainnet_url(),
        header_overrides_constructor: isthmus_header_overrides,
    },
}
