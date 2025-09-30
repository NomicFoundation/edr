// WARNING: This file is auto-generated. DO NOT EDIT MANUALLY.
// Any changes made to this file will be overwritten the next time it is
// generated. To make changes, update the generator script instead in
// `tools/src/op_chain_config.rs`.

use std::sync::LazyLock;

use edr_evm::hardfork::{self, Activations, ChainConfig, ForkCondition};
use op_revm::OpSpecId;

/// `swan` mainnet chain id
pub const MAINNET_CHAIN_ID: u64 = 0xFE;

/// `swan` mainnet chain configuration
pub static MAINNET_CONFIG: LazyLock<ChainConfig<OpSpecId>> = LazyLock::new(|| ChainConfig {
    name: "Swan Chain Mainnet".into(),
    base_fee_params: None,
    hardfork_activations: Activations::new(vec![hardfork::Activation {
        condition: ForkCondition::Timestamp(0),
        hardfork: OpSpecId::CANYON,
    }]),
});
