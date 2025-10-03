// WARNING: This file is auto-generated. DO NOT EDIT MANUALLY.
// Any changes made to this file will be overwritten the next time it is
// generated. To make changes, update the generator script instead in
// `tools/src/op_chain_config.rs`.

use edr_eip1559::{BaseFeeActivation, BaseFeeParams, ConstantBaseFeeParams, DynamicBaseFeeParams};
use edr_evm::hardfork::{self, Activations, ChainConfig, ForkCondition};
use op_revm::OpSpecId;

/// `Boba Mainnet` chain id
pub const MAINNET_CHAIN_ID: u64 = 0x120;

/// `Boba Mainnet` chain configuration
pub(crate) fn mainnet_config() -> ChainConfig<OpSpecId> {
    ChainConfig {
        name: "Boba Mainnet".into(),
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
                condition: ForkCondition::Block(0),
                hardfork: OpSpecId::REGOLITH,
            },
            hardfork::Activation {
                condition: ForkCondition::Timestamp(1713302879),
                hardfork: OpSpecId::CANYON,
            },
            hardfork::Activation {
                condition: ForkCondition::Timestamp(1713302880),
                hardfork: OpSpecId::ECOTONE,
            },
            hardfork::Activation {
                condition: ForkCondition::Timestamp(1725951600),
                hardfork: OpSpecId::FJORD,
            },
            hardfork::Activation {
                condition: ForkCondition::Timestamp(1729753200),
                hardfork: OpSpecId::GRANITE,
            },
            hardfork::Activation {
                condition: ForkCondition::Timestamp(1738785600),
                hardfork: OpSpecId::HOLOCENE,
            },
        ]),
    }
}

/// `Boba Sepolia Testnet` chain id
pub const SEPOLIA_CHAIN_ID: u64 = 0x70D2;

/// `Boba Sepolia Testnet` chain configuration
pub(crate) fn sepolia_config() -> ChainConfig<OpSpecId> {
    ChainConfig {
        name: "Boba Sepolia Testnet".into(),
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
                condition: ForkCondition::Block(0),
                hardfork: OpSpecId::REGOLITH,
            },
            hardfork::Activation {
                condition: ForkCondition::Timestamp(1705600788),
                hardfork: OpSpecId::CANYON,
            },
            hardfork::Activation {
                condition: ForkCondition::Timestamp(1709078400),
                hardfork: OpSpecId::ECOTONE,
            },
            hardfork::Activation {
                condition: ForkCondition::Timestamp(1722297600),
                hardfork: OpSpecId::FJORD,
            },
            hardfork::Activation {
                condition: ForkCondition::Timestamp(1726470000),
                hardfork: OpSpecId::GRANITE,
            },
            hardfork::Activation {
                condition: ForkCondition::Timestamp(1736150400),
                hardfork: OpSpecId::HOLOCENE,
            },
        ]),
    }
}
