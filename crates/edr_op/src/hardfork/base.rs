use std::sync::LazyLock;

use edr_evm::hardfork::{Activations, ChainConfig, ForkCondition};
use op_revm::OpSpecId;

/// Base Mainnet chain ID
pub const MAINNET_CHAIN_ID: u64 = 8453;

/// Base Mainnet chain config
///
/// <https://github.com/ethereum-optimism/superchain-registry/blob/51804a33655ddb4feeb0ad88960d9a81acdf6e62/superchain/configs/mainnet/base.toml>
pub static MAINNET_CONFIG: LazyLock<ChainConfig<OpSpecId>> = LazyLock::new(|| ChainConfig {
    name: "base-mainnet".into(),
    hardfork_activations: Activations::new(vec![
        (ForkCondition::Block(0), OpSpecId::BEDROCK),
        (ForkCondition::Block(0), OpSpecId::REGOLITH),
        (ForkCondition::Timestamp(1_704_992_401), OpSpecId::CANYON),
        (ForkCondition::Timestamp(1_710_374_401), OpSpecId::ECOTONE),
        (ForkCondition::Timestamp(1_720_627_201), OpSpecId::FJORD),
        (ForkCondition::Timestamp(1_726_070_401), OpSpecId::GRANITE),
        (ForkCondition::Timestamp(1_736_445_601), OpSpecId::HOLOCENE),
    ]),
});

/// Base Sepolia chain ID
pub const SEPOLIA_CHAIN_ID: u64 = 84532;

/// Base Sepolia chain config
///
/// <https://github.com/ethereum-optimism/superchain-registry/blob/51804a33655ddb4feeb0ad88960d9a81acdf6e62/superchain/configs/sepolia/base.toml>
pub static SEPOLIA_CONFIG: LazyLock<ChainConfig<OpSpecId>> = LazyLock::new(|| ChainConfig {
    name: "base-sepolia".into(),
    hardfork_activations: Activations::new(vec![
        (ForkCondition::Block(0), OpSpecId::BEDROCK),
        (ForkCondition::Block(0), OpSpecId::REGOLITH),
        (ForkCondition::Timestamp(1_699_981_200), OpSpecId::CANYON),
        (ForkCondition::Timestamp(1_708_534_800), OpSpecId::ECOTONE),
        (ForkCondition::Timestamp(1_716_998_400), OpSpecId::FJORD),
        (ForkCondition::Timestamp(1_723_478_400), OpSpecId::GRANITE),
        (ForkCondition::Timestamp(1_732_633_200), OpSpecId::HOLOCENE),
    ]),
});
