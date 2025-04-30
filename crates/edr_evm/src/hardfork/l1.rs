use std::sync::OnceLock;

use edr_eth::{HashMap, l1};

use super::{Activation, Activations, ChainConfig, ForkCondition};

const MAINNET_HARDFORKS: &[Activation<l1::SpecId>] = &[
    Activation {
        condition: ForkCondition::Block(0),
        hardfork: l1::SpecId::FRONTIER,
    },
    Activation {
        condition: ForkCondition::Block(200_000),
        hardfork: l1::SpecId::FRONTIER_THAWING,
    },
    Activation {
        condition: ForkCondition::Block(1_150_000),
        hardfork: l1::SpecId::HOMESTEAD,
    },
    Activation {
        condition: ForkCondition::Block(1_920_000),
        hardfork: l1::SpecId::DAO_FORK,
    },
    Activation {
        condition: ForkCondition::Block(2_463_000),
        hardfork: l1::SpecId::TANGERINE,
    },
    Activation {
        condition: ForkCondition::Block(2_675_000),
        hardfork: l1::SpecId::SPURIOUS_DRAGON,
    },
    Activation {
        condition: ForkCondition::Block(4_370_000),
        hardfork: l1::SpecId::BYZANTIUM,
    },
    Activation {
        condition: ForkCondition::Block(7_280_000),
        hardfork: l1::SpecId::CONSTANTINOPLE,
    },
    Activation {
        condition: ForkCondition::Block(7_280_000),
        hardfork: l1::SpecId::PETERSBURG,
    },
    Activation {
        condition: ForkCondition::Block(9_069_000),
        hardfork: l1::SpecId::ISTANBUL,
    },
    Activation {
        condition: ForkCondition::Block(9_200_000),
        hardfork: l1::SpecId::MUIR_GLACIER,
    },
    Activation {
        condition: ForkCondition::Block(12_244_000),
        hardfork: l1::SpecId::BERLIN,
    },
    Activation {
        condition: ForkCondition::Block(12_965_000),
        hardfork: l1::SpecId::LONDON,
    },
    Activation {
        condition: ForkCondition::Block(13_773_000),
        hardfork: l1::SpecId::ARROW_GLACIER,
    },
    Activation {
        condition: ForkCondition::Block(15_050_000),
        hardfork: l1::SpecId::GRAY_GLACIER,
    },
    Activation {
        condition: ForkCondition::Block(15_537_394),
        hardfork: l1::SpecId::MERGE,
    },
    Activation {
        condition: ForkCondition::Block(17_034_870),
        hardfork: l1::SpecId::SHANGHAI,
    },
    Activation {
        condition: ForkCondition::Block(19_426_589),
        hardfork: l1::SpecId::CANCUN,
    },
];

fn mainnet_config() -> &'static ChainConfig<l1::SpecId> {
    static CONFIG: OnceLock<ChainConfig<l1::SpecId>> = OnceLock::new();

    CONFIG.get_or_init(|| {
        let hardfork_activations = MAINNET_HARDFORKS.into();

        ChainConfig {
            name: "mainnet".to_string(),
            hardfork_activations,
        }
    })
}

const HOLESKY_HARDFORKS: &[Activation<l1::SpecId>] = &[
    Activation {
        condition: ForkCondition::Block(0),
        hardfork: l1::SpecId::MERGE,
    },
    Activation {
        condition: ForkCondition::Block(6_698),
        hardfork: l1::SpecId::SHANGHAI,
    },
    Activation {
        condition: ForkCondition::Block(894_733),
        hardfork: l1::SpecId::CANCUN,
    },
];

fn holesky_config() -> &'static ChainConfig<l1::SpecId> {
    static CONFIG: OnceLock<ChainConfig<l1::SpecId>> = OnceLock::new();

    CONFIG.get_or_init(|| {
        let hardfork_activations = HOLESKY_HARDFORKS.into();

        ChainConfig {
            name: "holesky".to_string(),
            hardfork_activations,
        }
    })
}

const SEPOLIA_HARDFORKS: &[Activation<l1::SpecId>] = &[
    Activation {
        condition: ForkCondition::Block(0),
        hardfork: l1::SpecId::LONDON,
    },
    Activation {
        condition: ForkCondition::Block(1_450_409),
        hardfork: l1::SpecId::MERGE,
    },
    Activation {
        condition: ForkCondition::Block(2_990_908),
        hardfork: l1::SpecId::SHANGHAI,
    },
    Activation {
        condition: ForkCondition::Block(5_187_023),
        hardfork: l1::SpecId::CANCUN,
    },
];

fn sepolia_config() -> &'static ChainConfig<l1::SpecId> {
    static CONFIG: OnceLock<ChainConfig<l1::SpecId>> = OnceLock::new();

    CONFIG.get_or_init(|| {
        let hardfork_activations = SEPOLIA_HARDFORKS.into();

        ChainConfig {
            name: "sepolia".to_string(),
            hardfork_activations,
        }
    })
}

fn chain_configs() -> &'static HashMap<u64, &'static ChainConfig<l1::SpecId>> {
    static CONFIGS: OnceLock<HashMap<u64, &'static ChainConfig<l1::SpecId>>> = OnceLock::new();

    CONFIGS.get_or_init(|| {
        let mut hardforks = HashMap::new();
        hardforks.insert(1, mainnet_config());
        hardforks.insert(17_000, holesky_config());
        hardforks.insert(11_155_111, sepolia_config());

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
pub fn chain_hardfork_activations(chain_id: u64) -> Option<&'static Activations<l1::SpecId>> {
    chain_configs()
        .get(&chain_id)
        .map(|config| &config.hardfork_activations)
}
