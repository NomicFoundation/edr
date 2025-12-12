//! Configurations for Ethereum L1 chains.

use std::sync::OnceLock;

use edr_chain_config::{ChainConfig, ForkCondition, HardforkActivation};
use edr_eip7892::ScheduledBlobParams;
use edr_primitives::HashMap;
pub use revm_primitives::hardfork::name;

use crate::{Hardfork, L1_BASE_FEE_PARAMS};

/// Mainnet chain ID
pub const L1_MAINNET_CHAIN_ID: u64 = 0x1;

const MAINNET_HARDFORKS: &[HardforkActivation<Hardfork>] = &[
    HardforkActivation {
        condition: ForkCondition::Block(0),
        hardfork: Hardfork::FRONTIER,
    },
    HardforkActivation {
        condition: ForkCondition::Block(200_000),
        hardfork: Hardfork::FRONTIER_THAWING,
    },
    HardforkActivation {
        condition: ForkCondition::Block(1_150_000),
        hardfork: Hardfork::HOMESTEAD,
    },
    HardforkActivation {
        condition: ForkCondition::Block(1_920_000),
        hardfork: Hardfork::DAO_FORK,
    },
    HardforkActivation {
        condition: ForkCondition::Block(2_463_000),
        hardfork: Hardfork::TANGERINE,
    },
    HardforkActivation {
        condition: ForkCondition::Block(2_675_000),
        hardfork: Hardfork::SPURIOUS_DRAGON,
    },
    HardforkActivation {
        condition: ForkCondition::Block(4_370_000),
        hardfork: Hardfork::BYZANTIUM,
    },
    HardforkActivation {
        condition: ForkCondition::Block(7_280_000),
        hardfork: Hardfork::CONSTANTINOPLE,
    },
    HardforkActivation {
        condition: ForkCondition::Block(7_280_000),
        hardfork: Hardfork::PETERSBURG,
    },
    HardforkActivation {
        condition: ForkCondition::Block(9_069_000),
        hardfork: Hardfork::ISTANBUL,
    },
    HardforkActivation {
        condition: ForkCondition::Block(9_200_000),
        hardfork: Hardfork::MUIR_GLACIER,
    },
    HardforkActivation {
        condition: ForkCondition::Block(12_244_000),
        hardfork: Hardfork::BERLIN,
    },
    HardforkActivation {
        condition: ForkCondition::Block(12_965_000),
        hardfork: Hardfork::LONDON,
    },
    HardforkActivation {
        condition: ForkCondition::Block(13_773_000),
        hardfork: Hardfork::ARROW_GLACIER,
    },
    HardforkActivation {
        condition: ForkCondition::Block(15_050_000),
        hardfork: Hardfork::GRAY_GLACIER,
    },
    HardforkActivation {
        condition: ForkCondition::Block(15_537_394),
        hardfork: Hardfork::MERGE,
    },
    HardforkActivation {
        condition: ForkCondition::Block(17_034_870),
        hardfork: Hardfork::SHANGHAI,
    },
    HardforkActivation {
        condition: ForkCondition::Block(19_426_589),
        hardfork: Hardfork::CANCUN,
    },
    HardforkActivation {
        condition: ForkCondition::Timestamp(1_746_612_311),
        hardfork: Hardfork::PRAGUE,
    },
    HardforkActivation {
        condition: ForkCondition::Timestamp(1_764_798_551),
        hardfork: Hardfork::OSAKA,
    },
];

fn mainnet_config() -> &'static ChainConfig<Hardfork> {
    static CONFIG: OnceLock<ChainConfig<Hardfork>> = OnceLock::new();

    CONFIG.get_or_init(|| {
        let hardfork_activations = MAINNET_HARDFORKS.into();

        ChainConfig {
            name: "Mainnet".to_owned(),
            hardfork_activations,
            base_fee_params: L1_BASE_FEE_PARAMS,
            bpo_hardfork_schedule: Some(ScheduledBlobParams::mainnet()),
        }
    })
}

/// Holesky chain ID
pub const HOLESKY_CHAIN_ID: u64 = 0x4268;

const HOLESKY_HARDFORKS: &[HardforkActivation<Hardfork>] = &[
    HardforkActivation {
        condition: ForkCondition::Block(0),
        hardfork: Hardfork::MERGE,
    },
    HardforkActivation {
        condition: ForkCondition::Block(6_698),
        hardfork: Hardfork::SHANGHAI,
    },
    HardforkActivation {
        condition: ForkCondition::Block(894_733),
        hardfork: Hardfork::CANCUN,
    },
    HardforkActivation {
        condition: ForkCondition::Timestamp(1_740_434_112),
        hardfork: Hardfork::PRAGUE,
    },
    HardforkActivation {
        condition: ForkCondition::Timestamp(1_759_308_480),
        hardfork: Hardfork::OSAKA,
    },
];

fn holesky_config() -> &'static ChainConfig<Hardfork> {
    static CONFIG: OnceLock<ChainConfig<Hardfork>> = OnceLock::new();

    CONFIG.get_or_init(|| {
        let hardfork_activations = HOLESKY_HARDFORKS.into();

        ChainConfig {
            name: "Holesky".to_owned(),
            hardfork_activations,
            base_fee_params: L1_BASE_FEE_PARAMS,
            bpo_hardfork_schedule: Some(ScheduledBlobParams::holesky()),
        }
    })
}

/// Hoodi chain ID
pub const HOODI_CHAIN_ID: u64 = 0x88bb0;

const HOODI_HARDFORKS: &[HardforkActivation<Hardfork>] = &[
    HardforkActivation {
        condition: ForkCondition::Block(0),
        hardfork: Hardfork::CANCUN,
    },
    HardforkActivation {
        condition: ForkCondition::Timestamp(1_742_999_832),
        hardfork: Hardfork::PRAGUE,
    },
    HardforkActivation {
        condition: ForkCondition::Timestamp(1_761_677_592),
        hardfork: Hardfork::OSAKA,
    },
];

fn hoodi_config() -> &'static ChainConfig<Hardfork> {
    static CONFIG: OnceLock<ChainConfig<Hardfork>> = OnceLock::new();

    CONFIG.get_or_init(|| {
        let hardfork_activations = HOODI_HARDFORKS.into();

        ChainConfig {
            name: "Hoodi".to_owned(),
            hardfork_activations,
            base_fee_params: L1_BASE_FEE_PARAMS,
            bpo_hardfork_schedule: Some(ScheduledBlobParams::hoodi()),
        }
    })
}

/// Sepolia chain ID
pub const SEPOLIA_CHAIN_ID: u64 = 0xaa36a7;

const SEPOLIA_HARDFORKS: &[HardforkActivation<Hardfork>] = &[
    HardforkActivation {
        condition: ForkCondition::Block(0),
        hardfork: Hardfork::LONDON,
    },
    HardforkActivation {
        condition: ForkCondition::Block(1_450_409),
        hardfork: Hardfork::MERGE,
    },
    HardforkActivation {
        condition: ForkCondition::Block(2_990_908),
        hardfork: Hardfork::SHANGHAI,
    },
    HardforkActivation {
        condition: ForkCondition::Block(5_187_023),
        hardfork: Hardfork::CANCUN,
    },
    HardforkActivation {
        condition: ForkCondition::Timestamp(1_741_159_776),
        hardfork: Hardfork::PRAGUE,
    },
    HardforkActivation {
        condition: ForkCondition::Timestamp(1_760_427_360),
        hardfork: Hardfork::OSAKA,
    },
];

fn sepolia_config() -> &'static ChainConfig<Hardfork> {
    static CONFIG: OnceLock<ChainConfig<Hardfork>> = OnceLock::new();

    CONFIG.get_or_init(|| {
        let hardfork_activations = SEPOLIA_HARDFORKS.into();

        ChainConfig {
            name: "Sepolia".to_owned(),
            hardfork_activations,
            base_fee_params: L1_BASE_FEE_PARAMS,
            bpo_hardfork_schedule: Some(ScheduledBlobParams::sepolia()),
        }
    })
}

pub(crate) fn l1_chain_configs() -> &'static HashMap<u64, ChainConfig<Hardfork>> {
    static CONFIGS: OnceLock<HashMap<u64, ChainConfig<Hardfork>>> = OnceLock::new();

    CONFIGS.get_or_init(|| {
        let mut hardforks = HashMap::default();
        hardforks.insert(L1_MAINNET_CHAIN_ID, mainnet_config().clone());
        hardforks.insert(HOLESKY_CHAIN_ID, holesky_config().clone());
        hardforks.insert(HOODI_CHAIN_ID, hoodi_config().clone());
        hardforks.insert(SEPOLIA_CHAIN_ID, sepolia_config().clone());

        hardforks
    })
}

/// Returns the corresponding configuration to the provided chain ID, if
/// it is supported.
pub fn l1_chain_config(chain_id: u64) -> Option<&'static ChainConfig<Hardfork>> {
    l1_chain_configs().get(&chain_id)
}
