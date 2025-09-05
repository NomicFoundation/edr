use std::{str::FromStr, sync::LazyLock};

use edr_evm::hardfork::{self, Activations, ChainConfig, ForkCondition};
use op_revm::OpSpecId;

pub const MAINNET_CHAIN_ID: u64 = 0x74C;

pub static MAINNET_CONFIG: LazyLock<ChainConfig<OpSpecId>> = LazyLock::new(|| ChainConfig {
    name: "Soneium".into(),
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
            condition: ForkCondition::Timestamp(0),
            hardfork: OpSpecId::from_str("fjord").unwrap(),
        },
        hardfork::Activation {
            condition: ForkCondition::Timestamp(0),
            hardfork: OpSpecId::from_str("granite").unwrap(),
        },
        hardfork::Activation {
            condition: ForkCondition::Timestamp(1738573200),
            hardfork: OpSpecId::from_str("holocene").unwrap(),
        },
        hardfork::Activation {
            condition: ForkCondition::Timestamp(1746806401),
            hardfork: OpSpecId::from_str("isthmus").unwrap(),
        },
    ]),
});
