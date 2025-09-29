// WARNING: This file is auto-generated. DO NOT EDIT MANUALLY.
// Any changes made to this file will be overwritten the next time it is
// generated. To make changes, update the generator script instead
// (tools/op_chain_config.rs).

use std::{str::FromStr, sync::LazyLock};

use edr_evm::hardfork::{self, Activations, ChainConfig, ForkCondition};
use op_revm::OpSpecId;

/// `mode` mainnet chain id
pub const MAINNET_CHAIN_ID: u64 = 0x868B;

/// `mode` mainnet chain configuration
pub static MAINNET_CONFIG: LazyLock<ChainConfig<OpSpecId>> = LazyLock::new(|| ChainConfig {
    name: "Mode".into(),
    base_fee_params: None,
    hardfork_activations: Activations::new(vec![
        hardfork::Activation {
            condition: ForkCondition::Timestamp(1704992401),
            hardfork: OpSpecId::from_str("canyon").unwrap(),
        },
        hardfork::Activation {
            condition: ForkCondition::Timestamp(1708560000),
            hardfork: OpSpecId::from_str("delta").unwrap(),
        },
        hardfork::Activation {
            condition: ForkCondition::Timestamp(1710374401),
            hardfork: OpSpecId::from_str("ecotone").unwrap(),
        },
        hardfork::Activation {
            condition: ForkCondition::Timestamp(1720627201),
            hardfork: OpSpecId::from_str("fjord").unwrap(),
        },
        hardfork::Activation {
            condition: ForkCondition::Timestamp(1726070401),
            hardfork: OpSpecId::from_str("granite").unwrap(),
        },
        hardfork::Activation {
            condition: ForkCondition::Timestamp(1736445601),
            hardfork: OpSpecId::from_str("holocene").unwrap(),
        },
        hardfork::Activation {
            condition: ForkCondition::Timestamp(1746806401),
            hardfork: OpSpecId::from_str("isthmus").unwrap(),
        },
    ]),
});
/// `mode` sepolia chain id
pub const SEPOLIA_CHAIN_ID: u64 = 0x397;

/// `mode` sepolia chain configuration
pub static SEPOLIA_CONFIG: LazyLock<ChainConfig<OpSpecId>> = LazyLock::new(|| ChainConfig {
    name: "Mode Testnet".into(),
    base_fee_params: None,
    hardfork_activations: Activations::new(vec![
        hardfork::Activation {
            condition: ForkCondition::Timestamp(1703203200),
            hardfork: OpSpecId::from_str("canyon").unwrap(),
        },
        hardfork::Activation {
            condition: ForkCondition::Timestamp(1703203200),
            hardfork: OpSpecId::from_str("delta").unwrap(),
        },
        hardfork::Activation {
            condition: ForkCondition::Timestamp(1708534800),
            hardfork: OpSpecId::from_str("ecotone").unwrap(),
        },
        hardfork::Activation {
            condition: ForkCondition::Timestamp(1716998400),
            hardfork: OpSpecId::from_str("fjord").unwrap(),
        },
        hardfork::Activation {
            condition: ForkCondition::Timestamp(1723478400),
            hardfork: OpSpecId::from_str("granite").unwrap(),
        },
        hardfork::Activation {
            condition: ForkCondition::Timestamp(1732633200),
            hardfork: OpSpecId::from_str("holocene").unwrap(),
        },
        hardfork::Activation {
            condition: ForkCondition::Timestamp(1742486400),
            hardfork: OpSpecId::from_str("pectra_blob_schedule").unwrap(),
        },
        hardfork::Activation {
            condition: ForkCondition::Timestamp(1744905600),
            hardfork: OpSpecId::from_str("isthmus").unwrap(),
        },
    ]),
});
