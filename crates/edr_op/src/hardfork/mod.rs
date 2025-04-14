use std::sync::OnceLock;

use edr_eth::HashMap;
use edr_evm::hardfork::{Activations, ChainConfig};
pub use op_revm::name;

use crate::OpSpecId;

/// Base chain configs
pub mod base;
/// OP chain configs
pub mod op;

// Source:
// <https://docs.optimism.io/builders/node-operators/network-upgrades>
fn chain_configs() -> &'static HashMap<u64, &'static ChainConfig<OpSpecId>> {
    static CONFIGS: OnceLock<HashMap<u64, &'static ChainConfig<OpSpecId>>> = OnceLock::new();

    CONFIGS.get_or_init(|| {
        let mut hardforks = HashMap::new();

        hardforks.insert(op::MAINNET_CHAIN_ID, &*op::MAINNET_CONFIG);
        hardforks.insert(op::SEPOLIA_CHAIN_ID, &*op::SEPOLIA_CONFIG);

        hardforks.insert(base::MAINNET_CHAIN_ID, &*base::MAINNET_CONFIG);
        hardforks.insert(base::SEPOLIA_CHAIN_ID, &*base::SEPOLIA_CONFIG);

        hardforks
    })
}

/// Returns the name corresponding to the provided chain ID, if it is supported.
pub fn chain_name(chain_id: u64) -> Option<&'static str> {
    chain_configs()
        .get(&chain_id)
        .map(|config| config.name.as_str())
}

/// Returns the hardfork activations corresponding to the provided chain ID, if
/// it is supported.
pub fn chain_hardfork_activations(chain_id: u64) -> Option<&'static Activations<OpSpecId>> {
    chain_configs()
        .get(&chain_id)
        .map(|config| &config.hardfork_activations)
}
