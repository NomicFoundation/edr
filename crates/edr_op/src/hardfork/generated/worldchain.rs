// WARNING: This file is auto-generated. DO NOT EDIT MANUALLY.
// Any changes made to this file will be overwritten the next time it is
// generated. To make changes, update the generator script instead in
// `crates/tool/op_chain_config_generator/src/op_chain_config.rs`.
//
// source: https://github.com/ethereum-optimism/superchain-registry/tree/0b03f5387c86c018343dc758c7b8913429a60c6b/superchain/configs

use edr_chain_config::{ChainConfig, ForkCondition, HardforkActivation, HardforkActivations};
use edr_eip1559::{BaseFeeActivation, BaseFeeParams, ConstantBaseFeeParams, DynamicBaseFeeParams};

use crate::hardfork::OpHardfork;

/// `World Chain` chain id
pub const MAINNET_CHAIN_ID: u64 = 0x1E0;

/// `World Chain` chain configuration
pub(super) fn mainnet_config() -> ChainConfig<OpHardfork> {
    ChainConfig {
        name: "World Chain".into(),
        base_fee_params: BaseFeeParams::Dynamic(DynamicBaseFeeParams::new(vec![
            (
                BaseFeeActivation::Hardfork(OpHardfork::Bedrock),
                ConstantBaseFeeParams::new(50, 10),
            ),
            (
                BaseFeeActivation::Hardfork(OpHardfork::Canyon),
                ConstantBaseFeeParams::new(250, 10),
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
                condition: ForkCondition::Timestamp(0),
                hardfork: OpHardfork::Canyon,
            },
            HardforkActivation {
                condition: ForkCondition::Timestamp(0),
                hardfork: OpHardfork::Ecotone,
            },
            HardforkActivation {
                condition: ForkCondition::Timestamp(1721826000),
                hardfork: OpHardfork::Fjord,
            },
            HardforkActivation {
                condition: ForkCondition::Timestamp(1727780400),
                hardfork: OpHardfork::Granite,
            },
            HardforkActivation {
                condition: ForkCondition::Timestamp(1738238400),
                hardfork: OpHardfork::Holocene,
            },
            HardforkActivation {
                condition: ForkCondition::Timestamp(1764072000),
                hardfork: OpHardfork::Isthmus,
            },
        ]),
        bpo_hardfork_schedule: None,
    }
}

/// `World Chain Sepolia Testnet` chain id
pub const SEPOLIA_CHAIN_ID: u64 = 0x12C1;

/// `World Chain Sepolia Testnet` chain configuration
pub(super) fn sepolia_config() -> ChainConfig<OpHardfork> {
    ChainConfig {
        name: "World Chain Sepolia Testnet".into(),
        base_fee_params: BaseFeeParams::Dynamic(DynamicBaseFeeParams::new(vec![
            (
                BaseFeeActivation::Hardfork(OpHardfork::Bedrock),
                ConstantBaseFeeParams::new(50, 10),
            ),
            (
                BaseFeeActivation::Hardfork(OpHardfork::Canyon),
                ConstantBaseFeeParams::new(250, 10),
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
                condition: ForkCondition::Timestamp(0),
                hardfork: OpHardfork::Canyon,
            },
            HardforkActivation {
                condition: ForkCondition::Timestamp(0),
                hardfork: OpHardfork::Ecotone,
            },
            HardforkActivation {
                condition: ForkCondition::Timestamp(1721739600),
                hardfork: OpHardfork::Fjord,
            },
            HardforkActivation {
                condition: ForkCondition::Timestamp(1726570800),
                hardfork: OpHardfork::Granite,
            },
            HardforkActivation {
                condition: ForkCondition::Timestamp(1737633600),
                hardfork: OpHardfork::Holocene,
            },
            HardforkActivation {
                condition: ForkCondition::Timestamp(1761825600),
                hardfork: OpHardfork::Isthmus,
            },
        ]),
        bpo_hardfork_schedule: None,
    }
}
