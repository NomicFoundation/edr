// WARNING: This file is auto-generated. DO NOT EDIT MANUALLY.
// Any changes made to this file will be overwritten the next time it is
// generated. To make changes, update the generator script instead in
// `tools/src/op_chain_config.rs`.

use std::sync::LazyLock;

use edr_eip1559::{BaseFeeActivation, BaseFeeParams, ConstantBaseFeeParams, DynamicBaseFeeParams};
use edr_evm::hardfork::{self, Activations, ChainConfig, ForkCondition};
use op_revm::OpSpecId;

/// `op` mainnet chain id
pub const MAINNET_CHAIN_ID: u64 = 0xA;

/// `op` mainnet chain configuration
pub static MAINNET_CONFIG: LazyLock<ChainConfig<OpSpecId>> = LazyLock::new(|| ChainConfig {
    name: "OP Mainnet".into(),
    base_fee_params: BaseFeeParams::Dynamic(DynamicBaseFeeParams::new(vec![
        (
            BaseFeeActivation::Hardfork(OpSpecId::BEDROCK),
            ConstantBaseFeeParams::new(50, 6),
        ),
        (
            BaseFeeActivation::Hardfork(OpSpecId::CANYON),
            ConstantBaseFeeParams::new(250, 6),
        ),
    ])),
    hardfork_activations: Activations::new(vec![
        hardfork::Activation {
            condition: ForkCondition::Timestamp(1704992401),
            hardfork: OpSpecId::CANYON,
        },
        hardfork::Activation {
            condition: ForkCondition::Timestamp(1710374401),
            hardfork: OpSpecId::ECOTONE,
        },
        hardfork::Activation {
            condition: ForkCondition::Timestamp(1720627201),
            hardfork: OpSpecId::FJORD,
        },
        hardfork::Activation {
            condition: ForkCondition::Timestamp(1726070401),
            hardfork: OpSpecId::GRANITE,
        },
        hardfork::Activation {
            condition: ForkCondition::Timestamp(1736445601),
            hardfork: OpSpecId::HOLOCENE,
        },
        hardfork::Activation {
            condition: ForkCondition::Timestamp(1746806401),
            hardfork: OpSpecId::ISTHMUS,
        },
    ]),
});
/// `op` sepolia chain id
pub const SEPOLIA_CHAIN_ID: u64 = 0xAA37DC;

/// `op` sepolia chain configuration
pub static SEPOLIA_CONFIG: LazyLock<ChainConfig<OpSpecId>> = LazyLock::new(|| ChainConfig {
    name: "OP Sepolia Testnet".into(),
    base_fee_params: BaseFeeParams::Dynamic(DynamicBaseFeeParams::new(vec![
        (
            BaseFeeActivation::Hardfork(OpSpecId::BEDROCK),
            ConstantBaseFeeParams::new(50, 6),
        ),
        (
            BaseFeeActivation::Hardfork(OpSpecId::CANYON),
            ConstantBaseFeeParams::new(250, 6),
        ),
    ])),
    hardfork_activations: Activations::new(vec![
        hardfork::Activation {
            condition: ForkCondition::Timestamp(1699981200),
            hardfork: OpSpecId::CANYON,
        },
        hardfork::Activation {
            condition: ForkCondition::Timestamp(1708534800),
            hardfork: OpSpecId::ECOTONE,
        },
        hardfork::Activation {
            condition: ForkCondition::Timestamp(1716998400),
            hardfork: OpSpecId::FJORD,
        },
        hardfork::Activation {
            condition: ForkCondition::Timestamp(1723478400),
            hardfork: OpSpecId::GRANITE,
        },
        hardfork::Activation {
            condition: ForkCondition::Timestamp(1732633200),
            hardfork: OpSpecId::HOLOCENE,
        },
        hardfork::Activation {
            condition: ForkCondition::Timestamp(1744905600),
            hardfork: OpSpecId::ISTHMUS,
        },
    ]),
});
