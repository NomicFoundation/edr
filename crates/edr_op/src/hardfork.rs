use std::sync::OnceLock;

use edr_eip1559::BaseFeeParams;
use edr_evm::hardfork::ChainConfig;
use edr_primitives::HashMap;
pub use op_revm::name;

use crate::Hardfork;

/// Base chain configs
pub mod base;
/// op stack chains configs generated
pub mod generated;
/// OP chain configs
pub mod op;

// Source:
// <https://docs.optimism.io/builders/node-operators/network-upgrades>
fn chain_configs() -> &'static HashMap<u64, &'static ChainConfig<Hardfork>> {
    static CONFIGS: OnceLock<HashMap<u64, &'static ChainConfig<Hardfork>>> = OnceLock::new();

    CONFIGS.get_or_init(|| {
        let mut hardforks = HashMap::new();

        hardforks.insert(op::MAINNET_CHAIN_ID, &*op::MAINNET_CONFIG);
        hardforks.insert(op::SEPOLIA_CHAIN_ID, &*op::SEPOLIA_CONFIG);

        hardforks.insert(base::MAINNET_CHAIN_ID, &*base::MAINNET_CONFIG);
        hardforks.insert(base::SEPOLIA_CHAIN_ID, &*base::SEPOLIA_CONFIG);

        hardforks
    })
}

/// Returns the corresponding configuration for the provided chain ID, if
/// it is supported.
pub fn chain_config(chain_id: u64) -> Option<&'static ChainConfig<Hardfork>> {
    chain_configs().get(&chain_id).copied()
}

/// Returns the default base fee params to fallback to
pub fn default_base_fee_params() -> &'static BaseFeeParams<Hardfork> {
    op::MAINNET_CONFIG
        .base_fee_params
        .as_ref()
        .expect("OP Mainnet should have the base fee params defined")
}
