// WARNING: This file is auto-generated. DO NOT EDIT MANUALLY.
// Any changes made to this file will be overwritten the next time it is
// generated. To make changes, update the generator script instead in
// `tools/src/op_chain_config.rs`.

use edr_eip1559::{BaseFeeActivation, BaseFeeParams, ConstantBaseFeeParams, DynamicBaseFeeParams};
use edr_evm::hardfork::{self, Activations, ChainConfig, ForkCondition};
use op_revm::OpSpecId;

/// `Shape` chain id
pub const MAINNET_CHAIN_ID: u64 = 0x168;

/// `Shape` chain configuration
pub(crate) fn mainnet_config() -> ChainConfig<OpSpecId> {
    ChainConfig {
        name: "Shape".into(),
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
                condition: ForkCondition::Timestamp(1727370000),
                hardfork: OpSpecId::GRANITE,
            },
        ]),
    }
}

/// `Shape Sepolia Testnet` chain id
pub const SEPOLIA_CHAIN_ID: u64 = 0x2B03;

/// `Shape Sepolia Testnet` chain configuration
pub(crate) fn sepolia_config() -> ChainConfig<OpSpecId> {
    ChainConfig {
        name: "Shape Sepolia Testnet".into(),
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
                condition: ForkCondition::Timestamp(1721732400),
                hardfork: OpSpecId::FJORD,
            },
            hardfork::Activation {
                condition: ForkCondition::Timestamp(1727197200),
                hardfork: OpSpecId::GRANITE,
            },
        ]),
    }
}
