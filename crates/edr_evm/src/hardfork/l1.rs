use std::sync::OnceLock;

use edr_eth::{HashMap, SpecId};

use super::{Activations, ChainConfig, ForkCondition};
use crate::chain_spec::L1ChainSpec;

const MAINNET_HARDFORKS: &[(ForkCondition, SpecId)] = &[
    (ForkCondition::Block(0), SpecId::FRONTIER),
    (ForkCondition::Block(200_000), SpecId::FRONTIER_THAWING),
    (ForkCondition::Block(1_150_000), SpecId::HOMESTEAD),
    (ForkCondition::Block(1_920_000), SpecId::DAO_FORK),
    (ForkCondition::Block(2_463_000), SpecId::TANGERINE),
    (ForkCondition::Block(2_675_000), SpecId::SPURIOUS_DRAGON),
    (ForkCondition::Block(4_370_000), SpecId::BYZANTIUM),
    (ForkCondition::Block(7_280_000), SpecId::CONSTANTINOPLE),
    (ForkCondition::Block(7_280_000), SpecId::PETERSBURG),
    (ForkCondition::Block(9_069_000), SpecId::ISTANBUL),
    (ForkCondition::Block(9_200_000), SpecId::MUIR_GLACIER),
    (ForkCondition::Block(12_244_000), SpecId::BERLIN),
    (ForkCondition::Block(12_965_000), SpecId::LONDON),
    (ForkCondition::Block(13_773_000), SpecId::ARROW_GLACIER),
    (ForkCondition::Block(15_050_000), SpecId::GRAY_GLACIER),
    (ForkCondition::Block(15_537_394), SpecId::MERGE),
    (ForkCondition::Block(17_034_870), SpecId::SHANGHAI),
    (ForkCondition::Block(19_426_589), SpecId::CANCUN),
];

fn mainnet_config() -> &'static ChainConfig<L1ChainSpec> {
    static CONFIG: OnceLock<ChainConfig<L1ChainSpec>> = OnceLock::new();

    CONFIG.get_or_init(|| {
        let hardfork_activations = MAINNET_HARDFORKS.into();

        ChainConfig {
            name: "mainnet".to_string(),
            hardfork_activations,
        }
    })
}

const ROPSTEN_HARDFORKS: &[(ForkCondition, SpecId)] = &[
    (ForkCondition::Block(1_700_000), SpecId::BYZANTIUM),
    (ForkCondition::Block(4_230_000), SpecId::CONSTANTINOPLE),
    (ForkCondition::Block(4_939_394), SpecId::PETERSBURG),
    (ForkCondition::Block(6_485_846), SpecId::ISTANBUL),
    (ForkCondition::Block(7_117_117), SpecId::MUIR_GLACIER),
    (ForkCondition::Block(9_812_189), SpecId::BERLIN),
    (ForkCondition::Block(10_499_401), SpecId::LONDON),
];

fn ropsten_config() -> &'static ChainConfig<L1ChainSpec> {
    static CONFIG: OnceLock<ChainConfig<L1ChainSpec>> = OnceLock::new();

    CONFIG.get_or_init(|| {
        let hardfork_activations = ROPSTEN_HARDFORKS.into();

        ChainConfig {
            name: "ropsten".to_string(),
            hardfork_activations,
        }
    })
}

const RINKEBY_HARDFORKS: &[(ForkCondition, SpecId)] = &[
    (ForkCondition::Block(1_035_301), SpecId::BYZANTIUM),
    (ForkCondition::Block(3_660_663), SpecId::CONSTANTINOPLE),
    (ForkCondition::Block(4_321_234), SpecId::PETERSBURG),
    (ForkCondition::Block(5_435_345), SpecId::ISTANBUL),
    (ForkCondition::Block(8_290_928), SpecId::BERLIN),
    (ForkCondition::Block(8_897_988), SpecId::LONDON),
];

fn rinkeby_config() -> &'static ChainConfig<L1ChainSpec> {
    static CONFIG: OnceLock<ChainConfig<L1ChainSpec>> = OnceLock::new();

    CONFIG.get_or_init(|| {
        let hardfork_activations = RINKEBY_HARDFORKS.into();

        ChainConfig {
            name: "rinkeby".to_string(),
            hardfork_activations,
        }
    })
}

const GOERLI_HARDFORKS: &[(ForkCondition, SpecId)] = &[
    (ForkCondition::Block(0), SpecId::PETERSBURG),
    (ForkCondition::Block(1_561_651), SpecId::ISTANBUL),
    (ForkCondition::Block(4_460_644), SpecId::BERLIN),
    (ForkCondition::Block(5_062_605), SpecId::LONDON),
    (ForkCondition::Block(7_382_818), SpecId::MERGE),
    (ForkCondition::Block(8_656_123), SpecId::SHANGHAI),
    (ForkCondition::Block(10_388_176), SpecId::CANCUN),
];

fn goerli_config() -> &'static ChainConfig<L1ChainSpec> {
    static CONFIG: OnceLock<ChainConfig<L1ChainSpec>> = OnceLock::new();

    CONFIG.get_or_init(|| {
        let hardfork_activations = GOERLI_HARDFORKS.into();

        ChainConfig {
            name: "goerli".to_string(),
            hardfork_activations,
        }
    })
}

const KOVAN_HARDFORKS: &[(ForkCondition, SpecId)] = &[
    (ForkCondition::Block(5_067_000), SpecId::BYZANTIUM),
    (ForkCondition::Block(9_200_000), SpecId::CONSTANTINOPLE),
    (ForkCondition::Block(10_255_201), SpecId::PETERSBURG),
    (ForkCondition::Block(14_111_141), SpecId::ISTANBUL),
    (ForkCondition::Block(24_770_900), SpecId::BERLIN),
    (ForkCondition::Block(26_741_100), SpecId::LONDON),
];

fn kovan_config() -> &'static ChainConfig<L1ChainSpec> {
    static CONFIG: OnceLock<ChainConfig<L1ChainSpec>> = OnceLock::new();

    CONFIG.get_or_init(|| {
        let hardfork_activations = KOVAN_HARDFORKS.into();

        ChainConfig {
            name: "kovan".to_string(),
            hardfork_activations,
        }
    })
}

const HOLESKY_HARDFORKS: &[(ForkCondition, SpecId)] = &[
    (ForkCondition::Block(0), SpecId::MERGE),
    (ForkCondition::Block(6_698), SpecId::SHANGHAI),
    (ForkCondition::Block(894_733), SpecId::CANCUN),
];

fn holesky_config() -> &'static ChainConfig<L1ChainSpec> {
    static CONFIG: OnceLock<ChainConfig<L1ChainSpec>> = OnceLock::new();

    CONFIG.get_or_init(|| {
        let hardfork_activations = HOLESKY_HARDFORKS.into();

        ChainConfig {
            name: "holesky".to_string(),
            hardfork_activations,
        }
    })
}

const SEPOLIA_HARDFORKS: &[(ForkCondition, SpecId)] = &[
    (ForkCondition::Block(0), SpecId::LONDON),
    (ForkCondition::Block(1_450_409), SpecId::MERGE),
    (ForkCondition::Block(2_990_908), SpecId::SHANGHAI),
    (ForkCondition::Block(5_187_023), SpecId::CANCUN),
];

fn sepolia_config() -> &'static ChainConfig<L1ChainSpec> {
    static CONFIG: OnceLock<ChainConfig<L1ChainSpec>> = OnceLock::new();

    CONFIG.get_or_init(|| {
        let hardfork_activations = SEPOLIA_HARDFORKS.into();

        ChainConfig {
            name: "sepolia".to_string(),
            hardfork_activations,
        }
    })
}

fn chain_configs() -> &'static HashMap<u64, &'static ChainConfig<L1ChainSpec>> {
    static CONFIGS: OnceLock<HashMap<u64, &'static ChainConfig<L1ChainSpec>>> = OnceLock::new();

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
pub fn chain_hardfork_activations(chain_id: u64) -> Option<&'static Activations<L1ChainSpec>> {
    chain_configs()
        .get(&chain_id)
        .map(|config| &config.hardfork_activations)
}
