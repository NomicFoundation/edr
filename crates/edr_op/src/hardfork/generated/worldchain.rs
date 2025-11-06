// WARNING: This file is auto-generated. DO NOT EDIT MANUALLY.
// Any changes made to this file will be overwritten the next time it is
// generated. To make changes, update the generator script instead in
// `crates/tool/op_chain_config_generator/src/op_chain_config.rs`.
//
// source: https://github.com/ethereum-optimism/superchain-registry/tree/f33efe105393d892365fda9fda42eb69951d011a/superchain/configs

use edr_chain_config::{ChainConfig, ForkCondition, HardforkActivation, HardforkActivations};
use edr_eip1559::{BaseFeeActivation, BaseFeeParams, ConstantBaseFeeParams, DynamicBaseFeeParams};
use op_revm::OpSpecId;

/// `World Chain` chain id
pub const MAINNET_CHAIN_ID: u64 = 0x1E0;

/// `World Chain` chain configuration
pub(super) fn mainnet_config() -> ChainConfig<OpSpecId> {
    ChainConfig {
        name: "World Chain".into(),
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
        hardfork_activations: HardforkActivations::new(vec![
            HardforkActivation {
                condition: ForkCondition::Timestamp(0),
                hardfork: OpSpecId::BEDROCK,
            },
            HardforkActivation {
                condition: ForkCondition::Timestamp(0),
                hardfork: OpSpecId::REGOLITH,
            },
            HardforkActivation {
                condition: ForkCondition::Timestamp(0),
                hardfork: OpSpecId::CANYON,
            },
            HardforkActivation {
                condition: ForkCondition::Timestamp(0),
                hardfork: OpSpecId::ECOTONE,
            },
            HardforkActivation {
                condition: ForkCondition::Timestamp(1721826000),
                hardfork: OpSpecId::FJORD,
            },
            HardforkActivation {
                condition: ForkCondition::Timestamp(1727780400),
                hardfork: OpSpecId::GRANITE,
            },
            HardforkActivation {
                condition: ForkCondition::Timestamp(1738238400),
                hardfork: OpSpecId::HOLOCENE,
            },
        ]),
    }
}

/// `World Chain Sepolia Testnet` chain id
pub const SEPOLIA_CHAIN_ID: u64 = 0x12C1;

/// `World Chain Sepolia Testnet` chain configuration
pub(super) fn sepolia_config() -> ChainConfig<OpSpecId> {
    ChainConfig {
        name: "World Chain Sepolia Testnet".into(),
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
        hardfork_activations: HardforkActivations::new(vec![
            HardforkActivation {
                condition: ForkCondition::Timestamp(0),
                hardfork: OpSpecId::BEDROCK,
            },
            HardforkActivation {
                condition: ForkCondition::Timestamp(0),
                hardfork: OpSpecId::REGOLITH,
            },
            HardforkActivation {
                condition: ForkCondition::Timestamp(0),
                hardfork: OpSpecId::CANYON,
            },
            HardforkActivation {
                condition: ForkCondition::Timestamp(0),
                hardfork: OpSpecId::ECOTONE,
            },
            HardforkActivation {
                condition: ForkCondition::Timestamp(1721739600),
                hardfork: OpSpecId::FJORD,
            },
            HardforkActivation {
                condition: ForkCondition::Timestamp(1726570800),
                hardfork: OpSpecId::GRANITE,
            },
            HardforkActivation {
                condition: ForkCondition::Timestamp(1737633600),
                hardfork: OpSpecId::HOLOCENE,
            },
            HardforkActivation {
                condition: ForkCondition::Timestamp(1761825600),
                hardfork: OpSpecId::ISTHMUS,
            },
        ]),
    }
}
