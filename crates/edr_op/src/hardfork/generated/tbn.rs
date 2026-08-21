// WARNING: This file is auto-generated. DO NOT EDIT MANUALLY.
// Any changes made to this file will be overwritten the next time it is
// generated. To make changes, update the generator script instead in
// `crates/tool/op_chain_config_generator/src/op_chain_config.rs`.
//
// source: https://github.com/ethereum-optimism/superchain-registry/tree/bb104b09fcd60fc01c8f8daf0f534aee88ff26de/superchain/configs

use edr_chain_config::{ChainConfig, ForkCondition, HardforkActivation, HardforkActivations};
use edr_eip1559::{BaseFeeActivation, BaseFeeParams, ConstantBaseFeeParams, DynamicBaseFeeParams};

use crate::hardfork::OpHardfork;

/// `Binary Mainnet` chain id
pub const MAINNET_CHAIN_ID: u64 = 0x270;

/// `Binary Mainnet` chain configuration
pub(super) fn mainnet_config() -> ChainConfig<OpHardfork> {
    ChainConfig {
        name: "Binary Mainnet".into(),
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
            HardforkActivation {
                condition: ForkCondition::Timestamp(1725536344),
                hardfork: OpHardfork::Fjord,
            },
            HardforkActivation {
                condition: ForkCondition::Timestamp(1726070401),
                hardfork: OpHardfork::Granite,
            },
            HardforkActivation {
                condition: ForkCondition::Timestamp(1736445601),
                hardfork: OpHardfork::Holocene,
            },
            HardforkActivation {
                condition: ForkCondition::Timestamp(1746806401),
                hardfork: OpHardfork::Isthmus,
            },
            HardforkActivation {
                condition: ForkCondition::Timestamp(1764691201),
                hardfork: OpHardfork::Jovian,
            },
        ]),
        bpo_hardfork_schedule: None,
    }
}

/// `Binary Sepolia` chain id
pub const SEPOLIA_CHAIN_ID: u64 = 0x271;

/// `Binary Sepolia` chain configuration
pub(super) fn sepolia_config() -> ChainConfig<OpHardfork> {
    ChainConfig {
        name: "Binary Sepolia".into(),
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
            HardforkActivation {
                condition: ForkCondition::Timestamp(1723204838),
                hardfork: OpHardfork::Fjord,
            },
            HardforkActivation {
                condition: ForkCondition::Timestamp(1723545055),
                hardfork: OpHardfork::Granite,
            },
        ]),
        bpo_hardfork_schedule: None,
    }
}
