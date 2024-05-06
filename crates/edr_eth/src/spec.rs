use std::sync::OnceLock;

use crate::{EthSpecId, HashMap};

/// A struct that stores the hardforks for a chain.
#[derive(Clone, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct HardforkActivations {
    /// (Start block number -> EthSpecId) mapping
    hardforks: Vec<(u64, EthSpecId)>,
}

impl HardforkActivations {
    /// Constructs a new instance with the provided hardforks.
    pub fn new(hardforks: Vec<(u64, EthSpecId)>) -> Self {
        Self { hardforks }
    }

    /// Creates a new instance for a new chain with the provided [`EthSpecId`].
    pub fn with_spec_id(spec_id: EthSpecId) -> Self {
        Self {
            hardforks: vec![(0, spec_id)],
        }
    }

    /// Whether no hardforks activations are present.
    pub fn is_empty(&self) -> bool {
        self.hardforks.is_empty()
    }

    /// Returns the hardfork's `EthSpecId` corresponding to the provided block
    /// number.
    pub fn hardfork_at_block_number(&self, block_number: u64) -> Option<EthSpecId> {
        self.hardforks
            .iter()
            .rev()
            .find(|(hardfork_number, _)| block_number >= *hardfork_number)
            .map(|entry| entry.1)
    }

    /// Retrieves the block number at which the provided hardfork was activated.
    pub fn hardfork_activation(&self, spec_id: EthSpecId) -> Option<u64> {
        self.hardforks
            .iter()
            .find(|(_, id)| *id == spec_id)
            .map(|(block, _)| *block)
    }
}

impl From<&[(u64, EthSpecId)]> for HardforkActivations {
    fn from(hardforks: &[(u64, EthSpecId)]) -> Self {
        Self {
            hardforks: hardforks.to_vec(),
        }
    }
}

struct ChainConfig {
    /// Chain name
    pub name: String,
    /// Hardfork activations for the chain
    pub hardfork_activations: HardforkActivations,
}

const MAINNET_HARDFORKS: &[(u64, EthSpecId)] = &[
    (0, EthSpecId::FRONTIER),
    (200_000, EthSpecId::FRONTIER_THAWING),
    (1_150_000, EthSpecId::HOMESTEAD),
    (1_920_000, EthSpecId::DAO_FORK),
    (2_463_000, EthSpecId::TANGERINE),
    (2_675_000, EthSpecId::SPURIOUS_DRAGON),
    (4_370_000, EthSpecId::BYZANTIUM),
    (7_280_000, EthSpecId::CONSTANTINOPLE),
    (7_280_000, EthSpecId::PETERSBURG),
    (9_069_000, EthSpecId::ISTANBUL),
    (9_200_000, EthSpecId::MUIR_GLACIER),
    (12_244_000, EthSpecId::BERLIN),
    (12_965_000, EthSpecId::LONDON),
    (13_773_000, EthSpecId::ARROW_GLACIER),
    (15_050_000, EthSpecId::GRAY_GLACIER),
    (15_537_394, EthSpecId::MERGE),
    (17_034_870, EthSpecId::SHANGHAI),
    (19_426_589, EthSpecId::CANCUN),
];

fn mainnet_config() -> &'static ChainConfig {
    static CONFIG: OnceLock<ChainConfig> = OnceLock::new();

    CONFIG.get_or_init(|| {
        let hardfork_activations = MAINNET_HARDFORKS.into();

        ChainConfig {
            name: "mainnet".to_string(),
            hardfork_activations,
        }
    })
}

const ROPSTEN_HARDFORKS: &[(u64, EthSpecId)] = &[
    (1_700_000, EthSpecId::BYZANTIUM),
    (4_230_000, EthSpecId::CONSTANTINOPLE),
    (4_939_394, EthSpecId::PETERSBURG),
    (6_485_846, EthSpecId::ISTANBUL),
    (7_117_117, EthSpecId::MUIR_GLACIER),
    (9_812_189, EthSpecId::BERLIN),
    (10_499_401, EthSpecId::LONDON),
];

fn ropsten_config() -> &'static ChainConfig {
    static CONFIG: OnceLock<ChainConfig> = OnceLock::new();

    CONFIG.get_or_init(|| {
        let hardfork_activations = ROPSTEN_HARDFORKS.into();

        ChainConfig {
            name: "ropsten".to_string(),
            hardfork_activations,
        }
    })
}

const RINKEBY_HARDFORKS: &[(u64, EthSpecId)] = &[
    (1_035_301, EthSpecId::BYZANTIUM),
    (3_660_663, EthSpecId::CONSTANTINOPLE),
    (4_321_234, EthSpecId::PETERSBURG),
    (5_435_345, EthSpecId::ISTANBUL),
    (8_290_928, EthSpecId::BERLIN),
    (8_897_988, EthSpecId::LONDON),
];

fn rinkeby_config() -> &'static ChainConfig {
    static CONFIG: OnceLock<ChainConfig> = OnceLock::new();

    CONFIG.get_or_init(|| {
        let hardfork_activations = RINKEBY_HARDFORKS.into();

        ChainConfig {
            name: "rinkeby".to_string(),
            hardfork_activations,
        }
    })
}

const GOERLI_HARDFORKS: &[(u64, EthSpecId)] = &[
    (0, EthSpecId::PETERSBURG),
    (1_561_651, EthSpecId::ISTANBUL),
    (4_460_644, EthSpecId::BERLIN),
    (5_062_605, EthSpecId::LONDON),
    (7_382_818, EthSpecId::MERGE),
    (8_656_123, EthSpecId::SHANGHAI),
    (10_388_176, EthSpecId::CANCUN),
];

fn goerli_config() -> &'static ChainConfig {
    static CONFIG: OnceLock<ChainConfig> = OnceLock::new();

    CONFIG.get_or_init(|| {
        let hardfork_activations = GOERLI_HARDFORKS.into();

        ChainConfig {
            name: "goerli".to_string(),
            hardfork_activations,
        }
    })
}

const KOVAN_HARDFORKS: &[(u64, EthSpecId)] = &[
    (5_067_000, EthSpecId::BYZANTIUM),
    (9_200_000, EthSpecId::CONSTANTINOPLE),
    (10_255_201, EthSpecId::PETERSBURG),
    (14_111_141, EthSpecId::ISTANBUL),
    (24_770_900, EthSpecId::BERLIN),
    (26_741_100, EthSpecId::LONDON),
];

fn kovan_config() -> &'static ChainConfig {
    static CONFIG: OnceLock<ChainConfig> = OnceLock::new();

    CONFIG.get_or_init(|| {
        let hardfork_activations = KOVAN_HARDFORKS.into();

        ChainConfig {
            name: "kovan".to_string(),
            hardfork_activations,
        }
    })
}

const HOLESKY_HARDFORKS: &[(u64, EthSpecId)] = &[
    (0, EthSpecId::MERGE),
    (6_698, EthSpecId::SHANGHAI),
    (894_733, EthSpecId::CANCUN),
];

fn holesky_config() -> &'static ChainConfig {
    static CONFIG: OnceLock<ChainConfig> = OnceLock::new();

    CONFIG.get_or_init(|| {
        let hardfork_activations = HOLESKY_HARDFORKS.into();

        ChainConfig {
            name: "holesky".to_string(),
            hardfork_activations,
        }
    })
}

const SEPOLIA_HARDFORKS: &[(u64, EthSpecId)] = &[
    (0, EthSpecId::LONDON),
    (1_450_409, EthSpecId::MERGE),
    (2_990_908, EthSpecId::SHANGHAI),
    (5_187_023, EthSpecId::CANCUN),
];

fn sepolia_config() -> &'static ChainConfig {
    static CONFIG: OnceLock<ChainConfig> = OnceLock::new();

    CONFIG.get_or_init(|| {
        let hardfork_activations = SEPOLIA_HARDFORKS.into();

        ChainConfig {
            name: "sepolia".to_string(),
            hardfork_activations,
        }
    })
}

fn chain_configs() -> &'static HashMap<u64, &'static ChainConfig> {
    static CONFIGS: OnceLock<HashMap<u64, &'static ChainConfig>> = OnceLock::new();

    CONFIGS.get_or_init(|| {
        let mut hardforks = HashMap::new();
        hardforks.insert(1, mainnet_config());
        hardforks.insert(3, ropsten_config());
        hardforks.insert(4, rinkeby_config());
        hardforks.insert(5, goerli_config());
        hardforks.insert(42, kovan_config());
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
pub fn chain_hardfork_activations(chain_id: u64) -> Option<&'static HardforkActivations> {
    chain_configs()
        .get(&chain_id)
        .map(|config| &config.hardfork_activations)
}
