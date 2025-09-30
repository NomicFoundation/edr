// WARNING: This file is auto-generated. DO NOT EDIT MANUALLY.
// Any changes made to this file will be overwritten the next time it is
// generated. To make changes, update the generator script instead in
// `tools/src/op_chain_config.rs`.

use std::sync::LazyLock;

use edr_eip1559::{BaseFeeActivation, BaseFeeParams, ConstantBaseFeeParams, DynamicBaseFeeParams};
use edr_evm::hardfork::{self, Activations, ChainConfig, ForkCondition};
use op_revm::OpSpecId;

/// `silent_data_mainnet` mainnet chain id
pub const MAINNET_CHAIN_ID: u64 = 0x5D001;

/// `silent_data_mainnet` mainnet chain configuration
pub static MAINNET_CONFIG: LazyLock<ChainConfig<OpSpecId>> = LazyLock::new(|| ChainConfig {
    name: "Silent Data Mainnet".into(),
    base_fee_params: BaseFeeParams::Dynamic(DynamicBaseFeeParams::new(vec![
        (
            BaseFeeActivation::Hardfork(OpSpecId::BEDROCK),
            ConstantBaseFeeParams::new(50, 6),
        ),
        (
            BaseFeeActivation::Hardfork(OpSpecId::CANYON),
            ConstantBaseFeeParams::new(250, 6),
        ),
    ])),
    hardfork_activations: Activations::new(vec![
        hardfork::Activation {
            condition: ForkCondition::Timestamp(0),
            hardfork: OpSpecId::CANYON,
        },
        hardfork::Activation {
            condition: ForkCondition::Timestamp(0),
            hardfork: OpSpecId::ECOTONE,
        },
        hardfork::Activation {
            condition: ForkCondition::Timestamp(0),
            hardfork: OpSpecId::FJORD,
        },
        hardfork::Activation {
            condition: ForkCondition::Timestamp(0),
            hardfork: OpSpecId::GRANITE,
        },
        hardfork::Activation {
            condition: ForkCondition::Timestamp(0),
            hardfork: OpSpecId::HOLOCENE,
        },
    ]),
});
