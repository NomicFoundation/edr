use std::sync::OnceLock;

use edr_eth::HashMap;
use edr_evm::hardfork::{Activations, ChainConfig};
pub use op_revm::name;

use crate::{chains, OpSpecId};

// Source:
// <https://docs.optimism.io/builders/node-operators/network-upgrades>
fn chain_configs() -> &'static HashMap<u64, &'static ChainConfig<OpSpecId>> {
    static CONFIGS: OnceLock<HashMap<u64, &'static ChainConfig<OpSpecId>>> = OnceLock::new();

    CONFIGS.get_or_init(|| {
        let mut hardforks = HashMap::new();

        hardforks.insert(chains::OP_MAINNET_CHAIN_ID, &*chains::OP_MAINNET_CONFIG);
        hardforks.insert(chains::OP_SEPOLIA_CHAIN_ID, &*chains::OP_SEPOLIA_CONFIG);

        hardforks.insert(chains::BASE_MAINNET_CHAIN_ID, &*chains::BASE_MAINNET_CONFIG);
        hardforks.insert(chains::BASE_SEPOLIA_CHAIN_ID, &*chains::BASE_SEPOLIA_CONFIG);

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
