// WARNING: This file is auto-generated. DO NOT EDIT MANUALLY.
// Any changes made to this file will be overwritten the next time it is
// generated. To make changes, update the generator script instead in
// `crates/tool/op_chain_config_generator/src/op_chain_config.rs`.
//
// source: https://github.com/ethereum-optimism/superchain-registry/tree/0b03f5387c86c018343dc758c7b8913429a60c6b/superchain/configs

use edr_chain_config::{ChainConfig, ForkCondition, HardforkActivation, HardforkActivations};
use edr_eip1559::{BaseFeeActivation, BaseFeeParams, ConstantBaseFeeParams, DynamicBaseFeeParams};

use crate::hardfork::OpHardfork;

/// `Mint Mainnet` chain id
pub const MAINNET_CHAIN_ID: u64 = 0xB9;

/// `Mint Mainnet` chain configuration
pub(super) fn mainnet_config() -> ChainConfig<OpHardfork> {
    ChainConfig {
        name: "Mint Mainnet".into(),
        base_fee_params: BaseFeeParams::Dynamic(DynamicBaseFeeParams::new(vec![
            (
                BaseFeeActivation::Hardfork(OpHardfork::BEDROCK),
                ConstantBaseFeeParams::new(50, 6),
            ),
            (
                BaseFeeActivation::Hardfork(OpHardfork::CANYON),
                ConstantBaseFeeParams::new(250, 6),
            ),
        ])),
        hardfork_activations: HardforkActivations::new(vec![
            HardforkActivation {
                condition: ForkCondition::Timestamp(0),
                hardfork: OpHardfork::BEDROCK,
            },
            HardforkActivation {
                condition: ForkCondition::Timestamp(0),
                hardfork: OpHardfork::REGOLITH,
            },
            HardforkActivation {
                condition: ForkCondition::Timestamp(0),
                hardfork: OpHardfork::CANYON,
            },
            HardforkActivation {
                condition: ForkCondition::Timestamp(0),
                hardfork: OpHardfork::ECOTONE,
            },
            HardforkActivation {
                condition: ForkCondition::Timestamp(1720627201),
                hardfork: OpHardfork::FJORD,
            },
            HardforkActivation {
                condition: ForkCondition::Timestamp(1726070401),
                hardfork: OpHardfork::GRANITE,
            },
            HardforkActivation {
                condition: ForkCondition::Timestamp(1736445601),
                hardfork: OpHardfork::HOLOCENE,
            },
            HardforkActivation {
                condition: ForkCondition::Timestamp(1746806401),
                hardfork: OpHardfork::ISTHMUS,
            },
        ]),
        bpo_hardfork_schedule: None,
    }
}
