use std::sync::OnceLock;

use edr_eth::{l1, HashMap};
use edr_evm::hardfork::{Activations, ChainConfig, ForkCondition};

use crate::{OpSpec, OpSpecId};

const MAINNET_HARDFORKS: &[(ForkCondition, OpSpec)] = &[
    (ForkCondition::Block(0), OpSpec::Eth(l1::SpecId::FRONTIER)),
    (ForkCondition::Block(0), OpSpec::Eth(l1::SpecId::HOMESTEAD)),
    (ForkCondition::Block(0), OpSpec::Eth(l1::SpecId::TANGERINE)),
    (
        ForkCondition::Block(0),
        OpSpec::Eth(l1::SpecId::SPURIOUS_DRAGON),
    ),
    (ForkCondition::Block(0), OpSpec::Eth(l1::SpecId::BYZANTIUM)),
    (
        ForkCondition::Block(0),
        OpSpec::Eth(l1::SpecId::CONSTANTINOPLE),
    ),
    (ForkCondition::Block(0), OpSpec::Eth(l1::SpecId::PETERSBURG)),
    (ForkCondition::Block(0), OpSpec::Eth(l1::SpecId::ISTANBUL)),
    (
        ForkCondition::Block(0),
        OpSpec::Eth(l1::SpecId::MUIR_GLACIER),
    ),
    (
        ForkCondition::Block(3_950_000),
        OpSpec::Eth(l1::SpecId::BERLIN),
    ),
    (
        ForkCondition::Block(105_235_063),
        OpSpec::Eth(l1::SpecId::LONDON),
    ),
    (
        ForkCondition::Block(105_235_063),
        OpSpec::Eth(l1::SpecId::ARROW_GLACIER),
    ),
    (
        ForkCondition::Block(105_235_063),
        OpSpec::Eth(l1::SpecId::GRAY_GLACIER),
    ),
    (
        ForkCondition::Block(105_235_063),
        OpSpec::Eth(l1::SpecId::MERGE),
    ),
    (
        ForkCondition::Block(105_235_063),
        OpSpec::Op(OpSpecId::BEDROCK),
    ),
    (
        ForkCondition::Block(105_235_063),
        OpSpec::Op(OpSpecId::REGOLITH),
    ),
    (
        ForkCondition::Timestamp(1_704_992_401),
        OpSpec::Eth(l1::SpecId::SHANGHAI),
    ),
    (
        ForkCondition::Timestamp(1_704_992_401),
        OpSpec::Op(OpSpecId::CANYON),
    ),
    (
        ForkCondition::Timestamp(1_710_374_401),
        OpSpec::Eth(l1::SpecId::CANCUN),
    ),
    (
        ForkCondition::Timestamp(1_710_374_401),
        OpSpec::Op(OpSpecId::ECOTONE),
    ),
    (
        ForkCondition::Timestamp(1_720_627_201),
        OpSpec::Op(OpSpecId::FJORD),
    ),
];

fn mainnet_config() -> &'static ChainConfig<OpSpec> {
    static CONFIG: OnceLock<ChainConfig<OpSpec>> = OnceLock::new();

    CONFIG.get_or_init(|| {
        let hardfork_activations = MAINNET_HARDFORKS.into();

        ChainConfig {
            name: "mainnet".to_string(),
            hardfork_activations,
        }
    })
}

const SEPOLIA_HARDFORKS: &[(ForkCondition, OpSpec)] = &[
    (ForkCondition::Block(0), OpSpec::Eth(l1::SpecId::FRONTIER)),
    (ForkCondition::Block(0), OpSpec::Eth(l1::SpecId::HOMESTEAD)),
    (ForkCondition::Block(0), OpSpec::Eth(l1::SpecId::TANGERINE)),
    (
        ForkCondition::Block(0),
        OpSpec::Eth(l1::SpecId::SPURIOUS_DRAGON),
    ),
    (ForkCondition::Block(0), OpSpec::Eth(l1::SpecId::BYZANTIUM)),
    (
        ForkCondition::Block(0),
        OpSpec::Eth(l1::SpecId::CONSTANTINOPLE),
    ),
    (ForkCondition::Block(0), OpSpec::Eth(l1::SpecId::PETERSBURG)),
    (ForkCondition::Block(0), OpSpec::Eth(l1::SpecId::ISTANBUL)),
    (
        ForkCondition::Block(0),
        OpSpec::Eth(l1::SpecId::MUIR_GLACIER),
    ),
    (ForkCondition::Block(0), OpSpec::Eth(l1::SpecId::BERLIN)),
    (ForkCondition::Block(0), OpSpec::Eth(l1::SpecId::LONDON)),
    (
        ForkCondition::Block(0),
        OpSpec::Eth(l1::SpecId::ARROW_GLACIER),
    ),
    (
        ForkCondition::Block(0),
        OpSpec::Eth(l1::SpecId::GRAY_GLACIER),
    ),
    (ForkCondition::Block(0), OpSpec::Eth(l1::SpecId::MERGE)),
    (ForkCondition::Block(0), OpSpec::Op(OpSpecId::BEDROCK)),
    (ForkCondition::Block(0), OpSpec::Op(OpSpecId::REGOLITH)),
    (
        ForkCondition::Timestamp(1_699_981_200),
        OpSpec::Eth(l1::SpecId::SHANGHAI),
    ),
    (
        ForkCondition::Timestamp(1_699_981_200),
        OpSpec::Op(OpSpecId::CANYON),
    ),
    (
        ForkCondition::Timestamp(1_708_534_800),
        OpSpec::Eth(l1::SpecId::CANCUN),
    ),
    (
        ForkCondition::Timestamp(1_708_534_800),
        OpSpec::Op(OpSpecId::ECOTONE),
    ),
    (
        ForkCondition::Timestamp(1_716_998_400),
        OpSpec::Op(OpSpecId::FJORD),
    ),
];

fn sepolia_config() -> &'static ChainConfig<OpSpec> {
    static CONFIG: OnceLock<ChainConfig<OpSpec>> = OnceLock::new();

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
fn chain_configs() -> &'static HashMap<u64, &'static ChainConfig<OpSpec>> {
    static CONFIGS: OnceLock<HashMap<u64, &'static ChainConfig<OpSpec>>> = OnceLock::new();

    CONFIGS.get_or_init(|| {
        let mut hardforks = HashMap::new();
        hardforks.insert(10, mainnet_config());
        hardforks.insert(11_155_420, sepolia_config());

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
pub fn chain_hardfork_activations(chain_id: u64) -> Option<&'static Activations<OpSpec>> {
    chain_configs()
        .get(&chain_id)
        .map(|config| &config.hardfork_activations)
}
