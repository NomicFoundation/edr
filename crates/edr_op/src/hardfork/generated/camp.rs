// WARNING: This file is auto-generated. DO NOT EDIT MANUALLY.
// Any changes made to this file will be overwritten the next time it is
// generated. To make changes, update the generator script instead in
// `crates/tool/op_chain_config_generator/src/op_chain_config.rs`.
//
// source: https://github.com/ethereum-optimism/superchain-registry/tree/f33efe105393d892365fda9fda42eb69951d011a/superchain/configs

use edr_chain_config::{ChainConfig, ForkCondition, HardforkActivation, HardforkActivations};
use edr_eip1559::{BaseFeeActivation, BaseFeeParams, ConstantBaseFeeParams, DynamicBaseFeeParams};
use op_revm::OpSpecId;

/// `Camp Network Testnet V2` chain id
pub const SEPOLIA_CHAIN_ID: u64 = 0x4F588;

/// `Camp Network Testnet V2` chain configuration
pub(super) fn sepolia_config() -> ChainConfig<OpSpecId> {
    ChainConfig {
        name: "Camp Network Testnet V2".into(),
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
                condition: ForkCondition::Timestamp(0),
                hardfork: OpSpecId::FJORD,
            },
            HardforkActivation {
                condition: ForkCondition::Timestamp(1723478400),
                hardfork: OpSpecId::GRANITE,
            },
            HardforkActivation {
                condition: ForkCondition::Timestamp(1732633200),
                hardfork: OpSpecId::HOLOCENE,
            },
        ]),
    }
}
