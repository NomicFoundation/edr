// WARNING: This file is auto-generated. DO NOT EDIT MANUALLY.
// Any changes made to this file will be overwritten the next time it is
// generated. To make changes, update the generator script instead in
// `crates/tool/op_chain_config_generator/src/op_chain_config.rs`.
//
// source: https://github.com/ethereum-optimism/superchain-registry/tree/bb104b09fcd60fc01c8f8daf0f534aee88ff26de/superchain/configs

use edr_chain_config::{ChainConfig, ForkCondition, HardforkActivation, HardforkActivations};
use edr_eip1559::{BaseFeeActivation, BaseFeeParams, ConstantBaseFeeParams, DynamicBaseFeeParams};

use crate::hardfork::OpHardfork;

/// `Boba Mainnet` chain id
pub const MAINNET_CHAIN_ID: u64 = 0x120;

/// `Boba Mainnet` chain configuration
pub(super) fn mainnet_config() -> ChainConfig<OpHardfork> {
    ChainConfig {
        name: "Boba Mainnet".into(),
        base_fee_params: BaseFeeParams::Dynamic(DynamicBaseFeeParams::new(vec![
            (
                BaseFeeActivation::Hardfork(OpHardfork::Bedrock),
                ConstantBaseFeeParams::new(50, 6),
            ),
            (
                BaseFeeActivation::Hardfork(OpHardfork::Canyon),
                ConstantBaseFeeParams::new(250, 6),
            ),
        ])),
        hardfork_activations: HardforkActivations::new(vec![
            HardforkActivation {
                condition: ForkCondition::Timestamp(0),
                hardfork: OpHardfork::Bedrock,
            },
            HardforkActivation {
                condition: ForkCondition::Timestamp(0),
                hardfork: OpHardfork::Regolith,
            },
            HardforkActivation {
                condition: ForkCondition::Timestamp(1713302879),
                hardfork: OpHardfork::Canyon,
            },
            HardforkActivation {
                condition: ForkCondition::Timestamp(1713302880),
                hardfork: OpHardfork::Ecotone,
            },
            HardforkActivation {
                condition: ForkCondition::Timestamp(1725951600),
                hardfork: OpHardfork::Fjord,
            },
            HardforkActivation {
                condition: ForkCondition::Timestamp(1729753200),
                hardfork: OpHardfork::Granite,
            },
            HardforkActivation {
                condition: ForkCondition::Timestamp(1738785600),
                hardfork: OpHardfork::Holocene,
            },
        ]),
        bpo_hardfork_schedule: None,
    }
}

/// `Boba Sepolia Testnet` chain id
pub const SEPOLIA_CHAIN_ID: u64 = 0x70D2;

/// `Boba Sepolia Testnet` chain configuration
pub(super) fn sepolia_config() -> ChainConfig<OpHardfork> {
    ChainConfig {
        name: "Boba Sepolia Testnet".into(),
        base_fee_params: BaseFeeParams::Dynamic(DynamicBaseFeeParams::new(vec![
            (
                BaseFeeActivation::Hardfork(OpHardfork::Bedrock),
                ConstantBaseFeeParams::new(50, 6),
            ),
            (
                BaseFeeActivation::Hardfork(OpHardfork::Canyon),
                ConstantBaseFeeParams::new(250, 6),
            ),
        ])),
        hardfork_activations: HardforkActivations::new(vec![
            HardforkActivation {
                condition: ForkCondition::Timestamp(0),
                hardfork: OpHardfork::Bedrock,
            },
            HardforkActivation {
                condition: ForkCondition::Timestamp(0),
                hardfork: OpHardfork::Regolith,
            },
            HardforkActivation {
                condition: ForkCondition::Timestamp(1705600788),
                hardfork: OpHardfork::Canyon,
            },
            HardforkActivation {
                condition: ForkCondition::Timestamp(1709078400),
                hardfork: OpHardfork::Ecotone,
            },
            HardforkActivation {
                condition: ForkCondition::Timestamp(1722297600),
                hardfork: OpHardfork::Fjord,
            },
            HardforkActivation {
                condition: ForkCondition::Timestamp(1726470000),
                hardfork: OpHardfork::Granite,
            },
            HardforkActivation {
                condition: ForkCondition::Timestamp(1736150400),
                hardfork: OpHardfork::Holocene,
            },
        ]),
        bpo_hardfork_schedule: None,
    }
}
