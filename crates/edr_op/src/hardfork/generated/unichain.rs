// WARNING: This file is auto-generated. DO NOT EDIT MANUALLY.
// Any changes made to this file will be overwritten the next time it is
// generated. To make changes, update the generator script instead in
// `crates/tool/op_chain_config_generator/src/op_chain_config.rs`.
//
// source: https://github.com/ethereum-optimism/superchain-registry/tree/0b03f5387c86c018343dc758c7b8913429a60c6b/superchain/configs

use edr_chain_config::{ChainConfig, ForkCondition, HardforkActivation, HardforkActivations};
use edr_eip1559::{BaseFeeActivation, BaseFeeParams, ConstantBaseFeeParams, DynamicBaseFeeParams};
use op_revm::OpSpecId;

/// `Unichain` chain id
pub const MAINNET_CHAIN_ID: u64 = 0x82;

/// `Unichain` chain configuration
pub(super) fn mainnet_config() -> ChainConfig<OpSpecId> {
    ChainConfig {
        name: "Unichain".into(),
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
                condition: ForkCondition::Timestamp(0),
                hardfork: OpSpecId::FJORD,
            },
            HardforkActivation {
                condition: ForkCondition::Timestamp(0),
                hardfork: OpSpecId::GRANITE,
            },
            HardforkActivation {
                condition: ForkCondition::Timestamp(1736445601),
                hardfork: OpSpecId::HOLOCENE,
            },
            HardforkActivation {
                condition: ForkCondition::Timestamp(1746806401),
                hardfork: OpSpecId::ISTHMUS,
            },
            HardforkActivation {
                condition: ForkCondition::Timestamp(1764691201),
                hardfork: OpSpecId::JOVIAN,
            },
        ]),
        bpo_hardfork_schedule: None,
    }
}

/// `Unichain Sepolia Testnet` chain id
pub const SEPOLIA_CHAIN_ID: u64 = 0x515;

/// `Unichain Sepolia Testnet` chain configuration
pub(super) fn sepolia_config() -> ChainConfig<OpSpecId> {
    ChainConfig {
        name: "Unichain Sepolia Testnet".into(),
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
                condition: ForkCondition::Timestamp(0),
                hardfork: OpSpecId::FJORD,
            },
            HardforkActivation {
                condition: ForkCondition::Timestamp(0),
                hardfork: OpSpecId::GRANITE,
            },
            HardforkActivation {
                condition: ForkCondition::Timestamp(1734559200),
                hardfork: OpSpecId::HOLOCENE,
            },
            HardforkActivation {
                condition: ForkCondition::Timestamp(1744905600),
                hardfork: OpSpecId::ISTHMUS,
            },
            HardforkActivation {
                condition: ForkCondition::Timestamp(1763568001),
                hardfork: OpSpecId::JOVIAN,
            },
        ]),
        bpo_hardfork_schedule: None,
    }
}
