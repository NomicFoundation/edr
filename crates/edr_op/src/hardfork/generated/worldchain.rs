// WARNING: This file is auto-generated. DO NOT EDIT MANUALLY.
// Any changes made to this file will be overwritten the next time it is
// generated. To make changes, update the generator script instead
// (tools/op_chain_config.rs).

use std::{str::FromStr, sync::LazyLock};

use edr_evm::hardfork::{self, Activations, ChainConfig, ForkCondition};
use op_revm::OpSpecId;

/// `worldchain` mainnet chain id
pub const MAINNET_CHAIN_ID: u64 = 0x1E0;

/// `worldchain` mainnet chain configuration
pub static MAINNET_CONFIG: LazyLock<ChainConfig<OpSpecId>> = LazyLock::new(|| ChainConfig {
    name: "World Chain".into(),
    base_fee_params: None,
    hardfork_activations: Activations::new(vec![
        hardfork::Activation {
            condition: ForkCondition::Timestamp(0),
            hardfork: OpSpecId::from_str("canyon").unwrap(),
        },
        hardfork::Activation {
            condition: ForkCondition::Timestamp(0),
            hardfork: OpSpecId::from_str("delta").unwrap(),
        },
        hardfork::Activation {
            condition: ForkCondition::Timestamp(0),
            hardfork: OpSpecId::from_str("ecotone").unwrap(),
        },
        hardfork::Activation {
            condition: ForkCondition::Timestamp(1721826000),
            hardfork: OpSpecId::from_str("fjord").unwrap(),
        },
        hardfork::Activation {
            condition: ForkCondition::Timestamp(1727780400),
            hardfork: OpSpecId::from_str("granite").unwrap(),
        },
        hardfork::Activation {
            condition: ForkCondition::Timestamp(1738238400),
            hardfork: OpSpecId::from_str("holocene").unwrap(),
        },
    ]),
});
/// `worldchain` sepolia chain id
pub const SEPOLIA_CHAIN_ID: u64 = 0x12C1;

/// `worldchain` sepolia chain configuration
pub static SEPOLIA_CONFIG: LazyLock<ChainConfig<OpSpecId>> = LazyLock::new(|| ChainConfig {
    name: "World Chain Sepolia Testnet".into(),
    base_fee_params: None,
    hardfork_activations: Activations::new(vec![
        hardfork::Activation {
            condition: ForkCondition::Timestamp(0),
            hardfork: OpSpecId::from_str("canyon").unwrap(),
        },
        hardfork::Activation {
            condition: ForkCondition::Timestamp(0),
            hardfork: OpSpecId::from_str("delta").unwrap(),
        },
        hardfork::Activation {
            condition: ForkCondition::Timestamp(0),
            hardfork: OpSpecId::from_str("ecotone").unwrap(),
        },
        hardfork::Activation {
            condition: ForkCondition::Timestamp(1721739600),
            hardfork: OpSpecId::from_str("fjord").unwrap(),
        },
        hardfork::Activation {
            condition: ForkCondition::Timestamp(1726570800),
            hardfork: OpSpecId::from_str("granite").unwrap(),
        },
        hardfork::Activation {
            condition: ForkCondition::Timestamp(1737633600),
            hardfork: OpSpecId::from_str("holocene").unwrap(),
        },
        hardfork::Activation {
            condition: ForkCondition::Timestamp(1742486400),
            hardfork: OpSpecId::from_str("pectra_blob_schedule").unwrap(),
        },
    ]),
});
