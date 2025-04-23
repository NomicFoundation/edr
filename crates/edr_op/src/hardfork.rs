use std::sync::OnceLock;

use edr_eth::HashMap;
use edr_evm::hardfork::{Activations, ChainConfig, ForkCondition};
pub use op_revm::name;

use crate::{MAINNET_CHAIN_ID, OpSpecId, SEPOLIA_CHAIN_ID};

// To update the hardforks, refer to the following link:
// <https://github.com/paradigmxyz/reth/blob/faedf98db3b1c965052b12b1663038a078807780/crates/optimism/hardforks/src/lib.rs>
const MAINNET_HARDFORKS: &[(ForkCondition, OpSpecId)] = &[
    (ForkCondition::Block(105_235_063), OpSpecId::BEDROCK),
    (ForkCondition::Block(105_235_063), OpSpecId::REGOLITH),
    (ForkCondition::Timestamp(1_704_992_401), OpSpecId::CANYON),
    (ForkCondition::Timestamp(1_710_374_401), OpSpecId::ECOTONE),
    (ForkCondition::Timestamp(1_720_627_201), OpSpecId::FJORD),
    (ForkCondition::Timestamp(1_726_070_401), OpSpecId::GRANITE),
    (ForkCondition::Timestamp(1_736_445_601), OpSpecId::HOLOCENE),
];

fn mainnet_config() -> &'static ChainConfig<OpSpecId> {
    static CONFIG: OnceLock<ChainConfig<OpSpecId>> = OnceLock::new();

    CONFIG.get_or_init(|| {
        let hardfork_activations = MAINNET_HARDFORKS.into();

        ChainConfig {
            name: "mainnet".to_string(),
            hardfork_activations,
        }
    })
}

const SEPOLIA_HARDFORKS: &[(ForkCondition, OpSpecId)] = &[
    (ForkCondition::Block(0), OpSpecId::BEDROCK),
    (ForkCondition::Block(0), OpSpecId::REGOLITH),
    (ForkCondition::Timestamp(1_699_981_200), OpSpecId::CANYON),
    (ForkCondition::Timestamp(1_708_534_800), OpSpecId::ECOTONE),
    (ForkCondition::Timestamp(1_716_998_400), OpSpecId::FJORD),
    (ForkCondition::Timestamp(1_723_478_400), OpSpecId::GRANITE),
    (ForkCondition::Timestamp(1_732_633_200), OpSpecId::HOLOCENE),
];

fn sepolia_config() -> &'static ChainConfig<OpSpecId> {
    static CONFIG: OnceLock<ChainConfig<OpSpecId>> = OnceLock::new();

    CONFIG.get_or_init(|| {
        let hardfork_activations = SEPOLIA_HARDFORKS.into();

        ChainConfig {
            name: "sepolia".to_string(),
            hardfork_activations,
        }
    })
}

// Source:
// <https://docs.optimism.io/builders/node-operators/network-upgrades>
fn chain_configs() -> &'static HashMap<u64, &'static ChainConfig<OpSpecId>> {
    static CONFIGS: OnceLock<HashMap<u64, &'static ChainConfig<OpSpecId>>> = OnceLock::new();

    CONFIGS.get_or_init(|| {
        let mut hardforks = HashMap::new();
        hardforks.insert(MAINNET_CHAIN_ID, mainnet_config());
        hardforks.insert(SEPOLIA_CHAIN_ID, sepolia_config());

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
