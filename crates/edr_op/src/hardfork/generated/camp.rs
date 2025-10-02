// WARNING: This file is auto-generated. DO NOT EDIT MANUALLY.
// Any changes made to this file will be overwritten the next time it is
// generated. To make changes, update the generator script instead in
// `tools/src/op_chain_config.rs`.

use edr_eip1559::{BaseFeeActivation, BaseFeeParams, ConstantBaseFeeParams, DynamicBaseFeeParams};
use edr_evm::hardfork::{self, Activations, ChainConfig, ForkCondition};
use op_revm::OpSpecId;

/// `Camp Network Testnet V2` chain id
pub const SEPOLIA_CHAIN_ID: u64 = 0x4F588;

/// `Camp Network Testnet V2` chain configuration
pub(crate) fn sepolia_config() -> ChainConfig<OpSpecId> {
    ChainConfig {
        name: "Camp Network Testnet V2".into(),
        base_fee_params: BaseFeeParams::Dynamic(DynamicBaseFeeParams::new(vec![
            (
                BaseFeeActivation::Hardfork(OpSpecId::BEDROCK),
                ConstantBaseFeeParams::new(50, 10),
            ),
            (
                BaseFeeActivation::Hardfork(OpSpecId::CANYON),
                ConstantBaseFeeParams::new(250, 10),
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
                condition: ForkCondition::Timestamp(1723478400),
                hardfork: OpSpecId::GRANITE,
            },
            hardfork::Activation {
                condition: ForkCondition::Timestamp(1732633200),
                hardfork: OpSpecId::HOLOCENE,
            },
        ]),
    }
}
