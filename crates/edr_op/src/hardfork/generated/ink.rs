// WARNING: This file is auto-generated. DO NOT EDIT MANUALLY.
// Any changes made to this file will be overwritten the next time it is
// generated. To make changes, update the generator script instead
// (tools/op_chain_config.rs).

use std::sync::LazyLock;

use edr_evm::hardfork::{self, Activations, ChainConfig, ForkCondition};
use op_revm::OpSpecId;

/// `ink` mainnet chain id
pub const MAINNET_CHAIN_ID: u64 = 0xDEF1;

/// `ink` mainnet chain configuration
pub static MAINNET_CONFIG: LazyLock<ChainConfig<OpSpecId>> = LazyLock::new(|| ChainConfig {
    name: "Ink".into(),
    base_fee_params: None,
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
            condition: ForkCondition::Timestamp(0),
            hardfork: OpSpecId::GRANITE,
        },
        hardfork::Activation {
            condition: ForkCondition::Timestamp(1742396400),
            hardfork: OpSpecId::HOLOCENE,
        },
        hardfork::Activation {
            condition: ForkCondition::Timestamp(1746806401),
            hardfork: OpSpecId::ISTHMUS,
        },
    ]),
});
/// `ink` sepolia chain id
pub const SEPOLIA_CHAIN_ID: u64 = 0xBA5ED;

/// `ink` sepolia chain configuration
pub static SEPOLIA_CONFIG: LazyLock<ChainConfig<OpSpecId>> = LazyLock::new(|| ChainConfig {
    name: "Ink Sepolia".into(),
    base_fee_params: None,
    hardfork_activations: Activations::new(vec![
        hardfork::Activation {
            condition: ForkCondition::Timestamp(1699981200),
            hardfork: OpSpecId::CANYON,
        },
        hardfork::Activation {
            condition: ForkCondition::Timestamp(1708534800),
            hardfork: OpSpecId::ECOTONE,
        },
        hardfork::Activation {
            condition: ForkCondition::Timestamp(1716998400),
            hardfork: OpSpecId::FJORD,
        },
        hardfork::Activation {
            condition: ForkCondition::Timestamp(1723478400),
            hardfork: OpSpecId::GRANITE,
        },
        hardfork::Activation {
            condition: ForkCondition::Timestamp(1732633200),
            hardfork: OpSpecId::HOLOCENE,
        },
        hardfork::Activation {
            condition: ForkCondition::Timestamp(1744905600),
            hardfork: OpSpecId::ISTHMUS,
        },
    ]),
});
