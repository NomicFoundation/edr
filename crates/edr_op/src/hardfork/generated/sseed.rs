// WARNING: This file is auto-generated. DO NOT EDIT MANUALLY.
// Any changes made to this file will be overwritten the next time it is
// generated. To make changes, update the generator script instead
// (tools/op_chain_config.rs).

use std::sync::LazyLock;

use edr_evm::hardfork::{self, Activations, ChainConfig, ForkCondition};
use op_revm::OpSpecId;

/// `sseed` mainnet chain id
pub const MAINNET_CHAIN_ID: u64 = 0x14D2;

/// `sseed` mainnet chain configuration
pub static MAINNET_CONFIG: LazyLock<ChainConfig<OpSpecId>> = LazyLock::new(|| ChainConfig {
    name: "Superseed".into(),
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
            condition: ForkCondition::Timestamp(1726185601),
            hardfork: OpSpecId::GRANITE,
        },
        hardfork::Activation {
            condition: ForkCondition::Timestamp(1736445601),
            hardfork: OpSpecId::HOLOCENE,
        },
        hardfork::Activation {
            condition: ForkCondition::Timestamp(1746806401),
            hardfork: OpSpecId::ISTHMUS,
        },
    ]),
});
