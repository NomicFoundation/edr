
use std::{str::FromStr, sync::LazyLock};

use edr_evm::hardfork::{self, Activations, ChainConfig, ForkCondition};
use op_revm::OpSpecId;

pub const MAINNET_CHAIN_ID: u64 = 0xFC;

pub static MAINNET_CONFIG: LazyLock<ChainConfig<OpSpecId>> = LazyLock::new(|| ChainConfig {
    name: "Fraxtal".into(),
    hardfork_activations: Activations::new( vec![
    
        hardfork::Activation {
            condition: ForkCondition::Timestamp(0),
            hardfork: OpSpecId::from_str("canyon").unwrap(),
        },

        hardfork::Activation {
            condition: ForkCondition::Timestamp(1717002001),
            hardfork: OpSpecId::from_str("delta").unwrap(),
        },

        hardfork::Activation {
            condition: ForkCondition::Timestamp(1717009201),
            hardfork: OpSpecId::from_str("ecotone").unwrap(),
        },

        hardfork::Activation {
            condition: ForkCondition::Timestamp(1733947201),
            hardfork: OpSpecId::from_str("fjord").unwrap(),
        },

        hardfork::Activation {
            condition: ForkCondition::Timestamp(1738958401),
            hardfork: OpSpecId::from_str("granite").unwrap(),
        },
   ]),
});