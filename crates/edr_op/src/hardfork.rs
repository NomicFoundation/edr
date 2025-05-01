use std::sync::{LazyLock, OnceLock};

use edr_eth::{HashMap, eips::eip1559::BaseFeeParams};
use edr_evm::hardfork::Activations;
pub use op_revm::name;

use crate::OpSpecId;

/// Base chain configs
pub mod base;
/// OP chain configs
pub mod op;

/// Type that stores the configuration for a chain.
pub struct OpChainConfig<HardforkT: 'static> {
    /// Chain name
    pub name: String,
    /// Hardfork activations for the chain
    pub hardfork_activations: Activations<HardforkT>,
    /// Base fee parameters for the chain
    pub base_fee_params: BaseFeeParams<HardforkT>,
}

// Source:
// <https://docs.optimism.io/builders/node-operators/network-upgrades>
fn chain_configs() -> &'static HashMap<u64, &'static LazyLock<OpChainConfig<OpSpecId>>> {
    static CONFIGS: OnceLock<HashMap<u64, &'static LazyLock<OpChainConfig<OpSpecId>>>> =
        OnceLock::new();

    CONFIGS.get_or_init(|| {
        let mut hardforks = HashMap::new();

        hardforks.insert(op::MAINNET_CHAIN_ID, &op::MAINNET_CONFIG);
        hardforks.insert(op::SEPOLIA_CHAIN_ID, &op::SEPOLIA_CONFIG);

        hardforks.insert(base::MAINNET_CHAIN_ID, &base::MAINNET_CONFIG);
        hardforks.insert(base::SEPOLIA_CHAIN_ID, &base::SEPOLIA_CONFIG);

        hardforks
    })
}

/// Returns the name corresponding to the provided chain ID, if it is supported
/// and known.
pub fn chain_name(chain_id: u64) -> Option<&'static str> {
    chain_configs()
        .get(&chain_id)
        .map(|config| config.name.as_str())
}

/// Returns the hardfork activations corresponding to the provided chain ID, if
/// it is supported and known.
pub fn chain_hardfork_activations(chain_id: u64) -> Option<&'static Activations<OpSpecId>> {
    chain_configs()
        .get(&chain_id)
        .map(|config| &config.hardfork_activations)
}

/// Returns the base fee params corresponding to the provided chain ID, if
/// it is supported and known.
pub fn chain_base_fee_params(chain_id: u64) -> Option<&'static BaseFeeParams<OpSpecId>> {
    chain_configs()
        .get(&chain_id)
        .map(|config| &config.base_fee_params)
}
