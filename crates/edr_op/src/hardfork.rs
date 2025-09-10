use std::sync::OnceLock;

use edr_eip1559::BaseFeeParams;
use edr_eth::HashMap;
use edr_evm::hardfork::{Activations, ChainConfig};
pub use op_revm::name;

use crate::Hardfork;

/// Base chain configs
pub mod base;
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

/// Returns the name corresponding to the provided chain ID, if it is supported.
pub fn chain_name(chain_id: u64) -> Option<&'static str> {
    chain_configs()
        .get(&chain_id)
        .map(|config| config.name.as_str())
}

/// Returns the hardfork activations corresponding to the provided chain ID, if
/// it is supported.
pub fn chain_hardfork_activations(chain_id: u64) -> Option<&'static Activations<Hardfork>> {
    chain_configs()
        .get(&chain_id)
        .map(|config| &config.hardfork_activations)
}

/// Returns the base fee params corresponding to the provided chain IF, if it is
/// supported if not, it default to chain type main chain values
pub fn chain_base_fee_params(chain_id: u64) -> &'static BaseFeeParams<Hardfork> {
    chain_configs()
        .get(&chain_id)
        .map_or(&op::MAINNET_CONFIG.base_fee_params, |config| {
            &config.base_fee_params
        })
}
