// WARNING: This file is auto-generated. DO NOT EDIT MANUALLY.
// Any changes made to this file will be overwritten the next time it is
// generated. To make changes, update the generator script instead
// (tools/op_chain_config.rs).

use std::{str::FromStr, sync::LazyLock};

use edr_evm::hardfork::{self, Activations, ChainConfig, ForkCondition};
use op_revm::OpSpecId;

/// `funki` mainnet chain id
pub const MAINNET_CHAIN_ID: u64 = 0x84BB;

/// `funki` mainnet chain configuration
pub static MAINNET_CONFIG: LazyLock<ChainConfig<OpSpecId>> = LazyLock::new(|| ChainConfig {
    name: "Funki".into(),
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
            condition: ForkCondition::Timestamp(0),
            hardfork: OpSpecId::from_str("fjord").unwrap(),
        },
    ]),
});
/// `funki` sepolia chain id
pub const SEPOLIA_CHAIN_ID: u64 = 0x33D90D;

/// `funki` sepolia chain configuration
pub static SEPOLIA_CONFIG: LazyLock<ChainConfig<OpSpecId>> = LazyLock::new(|| ChainConfig {
    name: "Funki Sepolia Testnet".into(),
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
            condition: ForkCondition::Timestamp(1744696800),
            hardfork: OpSpecId::from_str("pectra_blob_schedule").unwrap(),
        },
    ]),
});
