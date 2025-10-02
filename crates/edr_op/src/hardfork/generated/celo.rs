// WARNING: This file is auto-generated. DO NOT EDIT MANUALLY.
// Any changes made to this file will be overwritten the next time it is
// generated. To make changes, update the generator script instead in
// `tools/src/op_chain_config.rs`.

use edr_eip1559::{BaseFeeActivation, BaseFeeParams, ConstantBaseFeeParams, DynamicBaseFeeParams};
use edr_evm::hardfork::{self, Activations, ChainConfig, ForkCondition};
use op_revm::OpSpecId;

/// 'Celo' chain id
pub const MAINNET_CHAIN_ID: u64 = 0xA4EC;

/// 'Celo' chain configuration
pub(crate) fn mainnet_config() -> ChainConfig<OpSpecId> {
    ChainConfig {
        name: "Celo".into(),
        base_fee_params: BaseFeeParams::Dynamic(DynamicBaseFeeParams::new(vec![
            (
                BaseFeeActivation::Hardfork(OpSpecId::BEDROCK),
                ConstantBaseFeeParams::new(400, 5),
            ),
            (
                BaseFeeActivation::Hardfork(OpSpecId::CANYON),
                ConstantBaseFeeParams::new(400, 5),
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
                condition: ForkCondition::Timestamp(1752073200),
                hardfork: OpSpecId::HOLOCENE,
            },
            hardfork::Activation {
                condition: ForkCondition::Timestamp(1752073200),
                hardfork: OpSpecId::ISTHMUS,
            },
        ]),
    }
}
