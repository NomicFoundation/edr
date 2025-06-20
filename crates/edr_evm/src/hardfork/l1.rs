use std::sync::OnceLock;

use edr_eth::{l1, HashMap};

use super::{Activation, Activations, ChainConfig, ForkCondition};

/// Mainnet chain ID
pub const MAINNET_CHAIN_ID: u64 = 0x1;

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
    Activation {
        condition: ForkCondition::Timestamp(1_746_612_311),
        hardfork: l1::SpecId::PRAGUE,
    },
];

fn mainnet_config() -> &'static ChainConfig<l1::SpecId> {
    static CONFIG: OnceLock<ChainConfig<l1::SpecId>> = OnceLock::new();

    CONFIG.get_or_init(|| {
        let hardfork_activations = MAINNET_HARDFORKS.into();

        ChainConfig {
            name: "Mainnet".to_owned(),
            hardfork_activations,
        }
    })
}

/// Holesky chain ID
pub const HOLESKY_CHAIN_ID: u64 = 0x4268;

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
    Activation {
        condition: ForkCondition::Timestamp(1_740_434_112),
        hardfork: l1::SpecId::PRAGUE,
    },
];

fn holesky_config() -> &'static ChainConfig<l1::SpecId> {
    static CONFIG: OnceLock<ChainConfig<l1::SpecId>> = OnceLock::new();

    CONFIG.get_or_init(|| {
        let hardfork_activations = HOLESKY_HARDFORKS.into();

        ChainConfig {
            name: "Holesky".to_owned(),
            hardfork_activations,
        }
    })
}

/// Hoodi chain ID
pub const HOODI_CHAIN_ID: u64 = 0x88bb0;

const HOODI_HARDFORKS: &[Activation<l1::SpecId>] = &[
    Activation {
        condition: ForkCondition::Block(0),
        hardfork: l1::SpecId::CANCUN,
    },
    Activation {
        condition: ForkCondition::Timestamp(1_742_999_832),
        hardfork: l1::SpecId::PRAGUE,
    },
];

fn hoodi_config() -> &'static ChainConfig<l1::SpecId> {
    static CONFIG: OnceLock<ChainConfig<l1::SpecId>> = OnceLock::new();

    CONFIG.get_or_init(|| {
        let hardfork_activations = HOODI_HARDFORKS.into();

        ChainConfig {
            name: "Hoodi".to_owned(),
            hardfork_activations,
        }
    })
}

/// Sepolia chain ID
pub const SEPOLIA_CHAIN_ID: u64 = 0xaa36a7;

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
    Activation {
        condition: ForkCondition::Timestamp(1_741_159_776),
        hardfork: l1::SpecId::PRAGUE,
    },
];

fn sepolia_config() -> &'static ChainConfig<l1::SpecId> {
    static CONFIG: OnceLock<ChainConfig<l1::SpecId>> = OnceLock::new();

    CONFIG.get_or_init(|| {
        let hardfork_activations = SEPOLIA_HARDFORKS.into();

        ChainConfig {
            name: "Sepolia".to_owned(),
            hardfork_activations,
        }
    })
}

fn chain_configs() -> &'static HashMap<u64, &'static ChainConfig<l1::SpecId>> {
    static CONFIGS: OnceLock<HashMap<u64, &'static ChainConfig<l1::SpecId>>> = OnceLock::new();

    CONFIGS.get_or_init(|| {
        let mut hardforks = HashMap::new();
        hardforks.insert(MAINNET_CHAIN_ID, mainnet_config());
        hardforks.insert(HOLESKY_CHAIN_ID, holesky_config());
        hardforks.insert(HOODI_CHAIN_ID, hoodi_config());
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
pub fn chain_hardfork_activations(chain_id: u64) -> Option<&'static Activations<l1::SpecId>> {
    chain_configs()
        .get(&chain_id)
        .map(|config| &config.hardfork_activations)
}
