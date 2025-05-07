use std::sync::LazyLock;

use edr_evm::hardfork::{self, Activations, ChainConfig, ForkCondition};
use op_revm::OpSpecId;

/// OP Mainnet chain ID
pub const MAINNET_CHAIN_ID: u64 = 0xa;

/// OP Mainnet chain config
///
/// <https://github.com/ethereum-optimism/superchain-registry/blob/51804a33655ddb4feeb0ad88960d9a81acdf6e62/superchain/configs/mainnet/op.toml>
pub static MAINNET_CONFIG: LazyLock<ChainConfig<OpSpecId>> = LazyLock::new(|| ChainConfig {
    name: "OP Mainnet".into(),
    hardfork_activations: Activations::new(vec![
        hardfork::Activation {
            condition: ForkCondition::Block(105_235_063),
            hardfork: OpSpecId::BEDROCK,
        },
        hardfork::Activation {
            condition: ForkCondition::Block(105_235_063),
            hardfork: OpSpecId::REGOLITH,
        },
        hardfork::Activation {
            condition: ForkCondition::Timestamp(1_704_992_401),
            hardfork: OpSpecId::CANYON,
        },
        hardfork::Activation {
            condition: ForkCondition::Timestamp(1_710_374_401),
            hardfork: OpSpecId::ECOTONE,
        },
        hardfork::Activation {
            condition: ForkCondition::Timestamp(1_720_627_201),
            hardfork: OpSpecId::FJORD,
        },
        hardfork::Activation {
            condition: ForkCondition::Timestamp(1_726_070_401),
            hardfork: OpSpecId::GRANITE,
        },
        hardfork::Activation {
            condition: ForkCondition::Timestamp(1_736_445_601),
            hardfork: OpSpecId::HOLOCENE,
        },
    ]),
});

/// OP Sepolia chain ID
pub const SEPOLIA_CHAIN_ID: u64 = 0xaa37dc;

/// OP Sepolia chain config
///
/// <https://github.com/ethereum-optimism/superchain-registry/blob/51804a33655ddb4feeb0ad88960d9a81acdf6e62/superchain/configs/sepolia/op.toml>
pub static SEPOLIA_CONFIG: LazyLock<ChainConfig<OpSpecId>> = LazyLock::new(|| ChainConfig {
    name: "OP Sepolia".into(),
    hardfork_activations: Activations::new(vec![
        hardfork::Activation {
            condition: ForkCondition::Block(0),
            hardfork: OpSpecId::BEDROCK,
        },
        hardfork::Activation {
            condition: ForkCondition::Block(0),
            hardfork: OpSpecId::REGOLITH,
        },
        hardfork::Activation {
            condition: ForkCondition::Timestamp(1_699_981_200),
            hardfork: OpSpecId::CANYON,
        },
        hardfork::Activation {
            condition: ForkCondition::Timestamp(1_708_534_800),
            hardfork: OpSpecId::ECOTONE,
        },
        hardfork::Activation {
            condition: ForkCondition::Timestamp(1_716_998_400),
            hardfork: OpSpecId::FJORD,
        },
        hardfork::Activation {
            condition: ForkCondition::Timestamp(1_723_478_400),
            hardfork: OpSpecId::GRANITE,
        },
        hardfork::Activation {
            condition: ForkCondition::Timestamp(1_732_633_200),
            hardfork: OpSpecId::HOLOCENE,
        },
    ]),
});
