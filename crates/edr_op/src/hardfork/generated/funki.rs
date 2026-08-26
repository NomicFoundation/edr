// WARNING: This file is auto-generated. DO NOT EDIT MANUALLY.
// Any changes made to this file will be overwritten the next time it is
// generated. To make changes, update the generator script instead in
// `crates/tool/op_chain_config_generator/src/op_chain_config.rs`.
//
// source: https://github.com/ethereum-optimism/superchain-registry/tree/bb104b09fcd60fc01c8f8daf0f534aee88ff26de/superchain/configs

use edr_chain_config::{ChainConfig, ForkCondition, HardforkActivation, HardforkActivations};
use edr_eip1559::{BaseFeeActivation, BaseFeeParams, ConstantBaseFeeParams, DynamicBaseFeeParams};

use crate::hardfork::OpHardfork;

/// `Funki` chain id
pub const MAINNET_CHAIN_ID: u64 = 0x84BB;

/// `Funki` chain configuration
pub(super) fn mainnet_config() -> ChainConfig<OpHardfork> {
    ChainConfig {
        name: "Funki".into(),
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
                condition: ForkCondition::Timestamp(0),
                hardfork: OpHardfork::Fjord,
            },
            HardforkActivation {
                condition: ForkCondition::Timestamp(1767582000),
                hardfork: OpHardfork::Granite,
            },
            HardforkActivation {
                condition: ForkCondition::Timestamp(1767668400),
                hardfork: OpHardfork::Holocene,
            },
        ]),
        bpo_hardfork_schedule: None,
    }
}

/// `Funki Sepolia Testnet` chain id
pub const SEPOLIA_CHAIN_ID: u64 = 0x33D90D;

/// `Funki Sepolia Testnet` chain configuration
pub(super) fn sepolia_config() -> ChainConfig<OpHardfork> {
    ChainConfig {
        name: "Funki Sepolia Testnet".into(),
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
                condition: ForkCondition::Timestamp(0),
                hardfork: OpHardfork::Canyon,
            },
            HardforkActivation {
                condition: ForkCondition::Timestamp(0),
                hardfork: OpHardfork::Ecotone,
            },
        ]),
        bpo_hardfork_schedule: None,
    }
}
