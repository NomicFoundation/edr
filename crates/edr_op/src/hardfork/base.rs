use std::sync::LazyLock;

use edr_chain_config::{ChainConfig, ForkCondition, HardforkActivation, HardforkActivations};
use edr_eip1559::{BaseFeeActivation, BaseFeeParams, ConstantBaseFeeParams, DynamicBaseFeeParams};

use super::OpHardfork;

/// Base Mainnet chain ID
pub const MAINNET_CHAIN_ID: u64 = 8453;

pub(crate) static MAINNET_BASE_FEE_PARAMS: LazyLock<BaseFeeParams<OpHardfork>> =
    LazyLock::new(|| {
        BaseFeeParams::Dynamic(DynamicBaseFeeParams::new(vec![
            (
                BaseFeeActivation::Hardfork(OpHardfork::Bedrock),
                ConstantBaseFeeParams::new(50, 6),
            ),
            (
                BaseFeeActivation::Hardfork(OpHardfork::Canyon),
                ConstantBaseFeeParams::new(250, 6),
            ),
            (
                BaseFeeActivation::BlockNumber(25_955_889),
                ConstantBaseFeeParams::new(250, 2),
            ),
            (
                BaseFeeActivation::BlockNumber(30_795_009),
                ConstantBaseFeeParams::new(50, 2),
            ),
            (
                BaseFeeActivation::BlockNumber(31_747_084),
                ConstantBaseFeeParams::new(50, 3),
            ),
            (
                BaseFeeActivation::BlockNumber(37_483_302),
                ConstantBaseFeeParams::new(50, 4),
            ),
            (
                BaseFeeActivation::BlockNumber(38_088_319),
                ConstantBaseFeeParams::new(50, 5),
            ),
            (
                BaseFeeActivation::BlockNumber(39_647_879),
                ConstantBaseFeeParams::new(50, 6),
            ),
            (
                BaseFeeActivation::BlockNumber(41_711_238),
                ConstantBaseFeeParams::new(125, 6),
            ),
            (
                BaseFeeActivation::BlockNumber(43_841_215),
                ConstantBaseFeeParams::new(100, 5),
            ),
        ]))
    });

/// `Base` chain configuration.
///
/// Base was removed from the superchain registry
/// (<https://github.com/ethereum-optimism/superchain-registry/pull/1212>), so
/// its configuration is maintained manually here. Hardfork activations are
/// pinned from the last registry version that included them; new hardforks
/// must be added by hand.
pub(super) fn mainnet_config() -> ChainConfig<OpHardfork> {
    ChainConfig {
        name: "Base".into(),
        base_fee_params: MAINNET_BASE_FEE_PARAMS.clone(),
        hardfork_activations: HardforkActivations::new(vec![
            HardforkActivation {
                condition: ForkCondition::Timestamp(0),
                hardfork: OpHardfork::Bedrock,
            },
            HardforkActivation {
                condition: ForkCondition::Timestamp(0),
                hardfork: OpHardfork::Regolith,
            },
            HardforkActivation {
                condition: ForkCondition::Timestamp(1704992401),
                hardfork: OpHardfork::Canyon,
            },
            HardforkActivation {
                condition: ForkCondition::Timestamp(1710374401),
                hardfork: OpHardfork::Ecotone,
            },
            HardforkActivation {
                condition: ForkCondition::Timestamp(1720627201),
                hardfork: OpHardfork::Fjord,
            },
            HardforkActivation {
                condition: ForkCondition::Timestamp(1726070401),
                hardfork: OpHardfork::Granite,
            },
            HardforkActivation {
                condition: ForkCondition::Timestamp(1736445601),
                hardfork: OpHardfork::Holocene,
            },
            HardforkActivation {
                condition: ForkCondition::Timestamp(1746806401),
                hardfork: OpHardfork::Isthmus,
            },
            HardforkActivation {
                condition: ForkCondition::Timestamp(1764691201),
                hardfork: OpHardfork::Jovian,
            },
        ]),
        bpo_hardfork_schedule: None,
    }
}

/// Base Sepolia chain ID
pub const SEPOLIA_CHAIN_ID: u64 = 84532;

pub(crate) static SEPOLIA_BASE_FEE_PARAMS: LazyLock<BaseFeeParams<OpHardfork>> =
    LazyLock::new(|| {
        BaseFeeParams::Dynamic(DynamicBaseFeeParams::new(vec![
            (
                BaseFeeActivation::Hardfork(OpHardfork::Bedrock),
                ConstantBaseFeeParams::new(50, 10),
            ),
            (
                BaseFeeActivation::Hardfork(OpHardfork::Canyon),
                ConstantBaseFeeParams::new(250, 10),
            ),
            (
                BaseFeeActivation::BlockNumber(21_256_270),
                ConstantBaseFeeParams::new(250, 4),
            ),
            (
                BaseFeeActivation::BlockNumber(26_299_084),
                ConstantBaseFeeParams::new(50, 4),
            ),
        ]))
    });

/// `Base Sepolia Testnet` chain configuration.
///
/// Maintained manually — see [`mainnet_config`].
pub(super) fn sepolia_config() -> ChainConfig<OpHardfork> {
    ChainConfig {
        name: "Base Sepolia Testnet".into(),
        base_fee_params: SEPOLIA_BASE_FEE_PARAMS.clone(),
        hardfork_activations: HardforkActivations::new(vec![
            HardforkActivation {
                condition: ForkCondition::Timestamp(0),
                hardfork: OpHardfork::Bedrock,
            },
            HardforkActivation {
                condition: ForkCondition::Timestamp(0),
                hardfork: OpHardfork::Regolith,
            },
            HardforkActivation {
                condition: ForkCondition::Timestamp(1699981200),
                hardfork: OpHardfork::Canyon,
            },
            HardforkActivation {
                condition: ForkCondition::Timestamp(1708534800),
                hardfork: OpHardfork::Ecotone,
            },
            HardforkActivation {
                condition: ForkCondition::Timestamp(1716998400),
                hardfork: OpHardfork::Fjord,
            },
            HardforkActivation {
                condition: ForkCondition::Timestamp(1723478400),
                hardfork: OpHardfork::Granite,
            },
            HardforkActivation {
                condition: ForkCondition::Timestamp(1732633200),
                hardfork: OpHardfork::Holocene,
            },
            HardforkActivation {
                condition: ForkCondition::Timestamp(1744905600),
                hardfork: OpHardfork::Isthmus,
            },
            HardforkActivation {
                condition: ForkCondition::Timestamp(1763568001),
                hardfork: OpHardfork::Jovian,
            },
        ]),
        bpo_hardfork_schedule: None,
    }
}
