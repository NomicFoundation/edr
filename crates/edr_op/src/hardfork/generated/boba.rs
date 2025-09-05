use std::{str::FromStr, sync::LazyLock};

use edr_evm::hardfork::{self, Activations, ChainConfig, ForkCondition};
use op_revm::OpSpecId;

pub const MAINNET_CHAIN_ID: u64 = 0x120;

pub static MAINNET_CONFIG: LazyLock<ChainConfig<OpSpecId>> = LazyLock::new(|| ChainConfig {
    name: "Boba Mainnet".into(),
    hardfork_activations: Activations::new(vec![
        hardfork::Activation {
            condition: ForkCondition::Timestamp(1713302879),
            hardfork: OpSpecId::from_str("canyon").unwrap(),
        },
        hardfork::Activation {
            condition: ForkCondition::Timestamp(1713302879),
            hardfork: OpSpecId::from_str("delta").unwrap(),
        },
        hardfork::Activation {
            condition: ForkCondition::Timestamp(1713302880),
            hardfork: OpSpecId::from_str("ecotone").unwrap(),
        },
        hardfork::Activation {
            condition: ForkCondition::Timestamp(1725951600),
            hardfork: OpSpecId::from_str("fjord").unwrap(),
        },
        hardfork::Activation {
            condition: ForkCondition::Timestamp(1729753200),
            hardfork: OpSpecId::from_str("granite").unwrap(),
        },
        hardfork::Activation {
            condition: ForkCondition::Timestamp(1738785600),
            hardfork: OpSpecId::from_str("holocene").unwrap(),
        },
    ]),
});
pub const SEPOLIA_CHAIN_ID: u64 = 0x70D2;

pub static SEPOLIA_CONFIG: LazyLock<ChainConfig<OpSpecId>> = LazyLock::new(|| ChainConfig {
    name: "Boba Sepolia Testnet".into(),
    hardfork_activations: Activations::new(vec![
        hardfork::Activation {
            condition: ForkCondition::Timestamp(1705600788),
            hardfork: OpSpecId::from_str("canyon").unwrap(),
        },
        hardfork::Activation {
            condition: ForkCondition::Timestamp(1709078400),
            hardfork: OpSpecId::from_str("delta").unwrap(),
        },
        hardfork::Activation {
            condition: ForkCondition::Timestamp(1709078400),
            hardfork: OpSpecId::from_str("ecotone").unwrap(),
        },
        hardfork::Activation {
            condition: ForkCondition::Timestamp(1722297600),
            hardfork: OpSpecId::from_str("fjord").unwrap(),
        },
        hardfork::Activation {
            condition: ForkCondition::Timestamp(1726470000),
            hardfork: OpSpecId::from_str("granite").unwrap(),
        },
        hardfork::Activation {
            condition: ForkCondition::Timestamp(1736150400),
            hardfork: OpSpecId::from_str("holocene").unwrap(),
        },
        hardfork::Activation {
            condition: ForkCondition::Timestamp(1743534000),
            hardfork: OpSpecId::from_str("pectra_blob_schedule").unwrap(),
        },
    ]),
});
