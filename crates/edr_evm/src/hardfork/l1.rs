use std::sync::OnceLock;

use edr_eip1559::{BaseFeeParams, ConstantBaseFeeParams};
use edr_eth::HashMap;

use super::{Activation, Activations, ChainConfig, ForkCondition};

/// Mainnet chain ID
pub const MAINNET_CHAIN_ID: u64 = 0x1;

const BASE_FEE_PARAMS: BaseFeeParams<edr_chain_l1::Hardfork> =
    BaseFeeParams::Constant(ConstantBaseFeeParams::ethereum());

const MAINNET_HARDFORKS: &[Activation<edr_chain_l1::Hardfork>] = &[
    Activation {
        condition: ForkCondition::Block(0),
        hardfork: edr_chain_l1::Hardfork::FRONTIER,
    },
    Activation {
        condition: ForkCondition::Block(200_000),
        hardfork: edr_chain_l1::Hardfork::FRONTIER_THAWING,
    },
    Activation {
        condition: ForkCondition::Block(1_150_000),
        hardfork: edr_chain_l1::Hardfork::HOMESTEAD,
    },
    Activation {
        condition: ForkCondition::Block(1_920_000),
        hardfork: edr_chain_l1::Hardfork::DAO_FORK,
    },
    Activation {
        condition: ForkCondition::Block(2_463_000),
        hardfork: edr_chain_l1::Hardfork::TANGERINE,
    },
    Activation {
        condition: ForkCondition::Block(2_675_000),
        hardfork: edr_chain_l1::Hardfork::SPURIOUS_DRAGON,
    },
    Activation {
        condition: ForkCondition::Block(4_370_000),
        hardfork: edr_chain_l1::Hardfork::BYZANTIUM,
    },
    Activation {
        condition: ForkCondition::Block(7_280_000),
        hardfork: edr_chain_l1::Hardfork::CONSTANTINOPLE,
    },
    Activation {
        condition: ForkCondition::Block(7_280_000),
        hardfork: edr_chain_l1::Hardfork::PETERSBURG,
    },
    Activation {
        condition: ForkCondition::Block(9_069_000),
        hardfork: edr_chain_l1::Hardfork::ISTANBUL,
    },
    Activation {
        condition: ForkCondition::Block(9_200_000),
        hardfork: edr_chain_l1::Hardfork::MUIR_GLACIER,
    },
    Activation {
        condition: ForkCondition::Block(12_244_000),
        hardfork: edr_chain_l1::Hardfork::BERLIN,
    },
    Activation {
        condition: ForkCondition::Block(12_965_000),
        hardfork: edr_chain_l1::Hardfork::LONDON,
    },
    Activation {
        condition: ForkCondition::Block(13_773_000),
        hardfork: edr_chain_l1::Hardfork::ARROW_GLACIER,
    },
    Activation {
        condition: ForkCondition::Block(15_050_000),
        hardfork: edr_chain_l1::Hardfork::GRAY_GLACIER,
    },
    Activation {
        condition: ForkCondition::Block(15_537_394),
        hardfork: edr_chain_l1::Hardfork::MERGE,
    },
    Activation {
        condition: ForkCondition::Block(17_034_870),
        hardfork: edr_chain_l1::Hardfork::SHANGHAI,
    },
    Activation {
        condition: ForkCondition::Block(19_426_589),
        hardfork: edr_chain_l1::Hardfork::CANCUN,
    },
    Activation {
        condition: ForkCondition::Timestamp(1_746_612_311),
        hardfork: edr_chain_l1::Hardfork::PRAGUE,
    },
];

fn mainnet_config() -> &'static ChainConfig<edr_chain_l1::Hardfork> {
    static CONFIG: OnceLock<ChainConfig<edr_chain_l1::Hardfork>> = OnceLock::new();

    CONFIG.get_or_init(|| {
        let hardfork_activations = MAINNET_HARDFORKS.into();

        ChainConfig {
            name: "Mainnet".to_owned(),
            hardfork_activations,
            base_fee_params: BASE_FEE_PARAMS,
        }
    })
}

/// Holesky chain ID
pub const HOLESKY_CHAIN_ID: u64 = 0x4268;

const HOLESKY_HARDFORKS: &[Activation<edr_chain_l1::Hardfork>] = &[
    Activation {
        condition: ForkCondition::Block(0),
        hardfork: edr_chain_l1::Hardfork::MERGE,
    },
    Activation {
        condition: ForkCondition::Block(6_698),
        hardfork: edr_chain_l1::Hardfork::SHANGHAI,
    },
    Activation {
        condition: ForkCondition::Block(894_733),
        hardfork: edr_chain_l1::Hardfork::CANCUN,
    },
    Activation {
        condition: ForkCondition::Timestamp(1_740_434_112),
        hardfork: edr_chain_l1::Hardfork::PRAGUE,
    },
];

fn holesky_config() -> &'static ChainConfig<edr_chain_l1::Hardfork> {
    static CONFIG: OnceLock<ChainConfig<edr_chain_l1::Hardfork>> = OnceLock::new();

    CONFIG.get_or_init(|| {
        let hardfork_activations = HOLESKY_HARDFORKS.into();

        ChainConfig {
            name: "Holesky".to_owned(),
            hardfork_activations,
            base_fee_params: BASE_FEE_PARAMS,
        }
    })
}

/// Hoodi chain ID
pub const HOODI_CHAIN_ID: u64 = 0x88bb0;

const HOODI_HARDFORKS: &[Activation<edr_chain_l1::Hardfork>] = &[
    Activation {
        condition: ForkCondition::Block(0),
        hardfork: edr_chain_l1::Hardfork::CANCUN,
    },
    Activation {
        condition: ForkCondition::Timestamp(1_742_999_832),
        hardfork: edr_chain_l1::Hardfork::PRAGUE,
    },
];

fn hoodi_config() -> &'static ChainConfig<edr_chain_l1::Hardfork> {
    static CONFIG: OnceLock<ChainConfig<edr_chain_l1::Hardfork>> = OnceLock::new();

    CONFIG.get_or_init(|| {
        let hardfork_activations = HOODI_HARDFORKS.into();

        ChainConfig {
            name: "Hoodi".to_owned(),
            hardfork_activations,
            base_fee_params: BASE_FEE_PARAMS,
        }
    })
}

/// Sepolia chain ID
pub const SEPOLIA_CHAIN_ID: u64 = 0xaa36a7;

const SEPOLIA_HARDFORKS: &[Activation<edr_chain_l1::Hardfork>] = &[
    Activation {
        condition: ForkCondition::Block(0),
        hardfork: edr_chain_l1::Hardfork::LONDON,
    },
    Activation {
        condition: ForkCondition::Block(1_450_409),
        hardfork: edr_chain_l1::Hardfork::MERGE,
    },
    Activation {
        condition: ForkCondition::Block(2_990_908),
        hardfork: edr_chain_l1::Hardfork::SHANGHAI,
    },
    Activation {
        condition: ForkCondition::Block(5_187_023),
        hardfork: edr_chain_l1::Hardfork::CANCUN,
    },
    Activation {
        condition: ForkCondition::Timestamp(1_741_159_776),
        hardfork: edr_chain_l1::Hardfork::PRAGUE,
    },
];

fn sepolia_config() -> &'static ChainConfig<edr_chain_l1::Hardfork> {
    static CONFIG: OnceLock<ChainConfig<edr_chain_l1::Hardfork>> = OnceLock::new();

    CONFIG.get_or_init(|| {
        let hardfork_activations = SEPOLIA_HARDFORKS.into();

        ChainConfig {
            name: "Sepolia".to_owned(),
            hardfork_activations,
            base_fee_params: BASE_FEE_PARAMS,
        }
    })
}

fn chain_configs() -> &'static HashMap<u64, &'static ChainConfig<edr_chain_l1::Hardfork>> {
    static CONFIGS: OnceLock<HashMap<u64, &'static ChainConfig<edr_chain_l1::Hardfork>>> =
        OnceLock::new();

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
pub fn chain_hardfork_activations(
    chain_id: u64,
) -> Option<&'static Activations<edr_chain_l1::Hardfork>> {
    chain_configs()
        .get(&chain_id)
        .map(|config| &config.hardfork_activations)
}

/// Returns the hardfork activations corresponding to the provided chain ID, if
/// it is supported. If not, it defaults to Mainnet values
pub fn chain_base_fee_params(chain_id: u64) -> &'static BaseFeeParams<edr_chain_l1::Hardfork> {
    chain_configs()
        .get(&chain_id)
        .map_or(&mainnet_config().base_fee_params, |config| {
            &config.base_fee_params
        })
}
