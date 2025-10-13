use std::sync::LazyLock;

use edr_eip1559::{BaseFeeActivation, BaseFeeParams, ConstantBaseFeeParams, DynamicBaseFeeParams};
use op_revm::OpSpecId;

/// OP Mainnet chain ID
pub const MAINNET_CHAIN_ID: u64 = 0xa;

pub(crate) static MAINNET_BASE_FEE_PARAMS: LazyLock<BaseFeeParams<OpSpecId>> =
    LazyLock::new(|| {
        BaseFeeParams::Dynamic(DynamicBaseFeeParams::new(vec![
            (
                BaseFeeActivation::Hardfork(OpSpecId::BEDROCK),
                ConstantBaseFeeParams::new(50, 6),
            ),
            (
                BaseFeeActivation::Hardfork(OpSpecId::CANYON),
                ConstantBaseFeeParams::new(250, 6),
            ),
            (
                BaseFeeActivation::BlockNumber(135_513_416),
                ConstantBaseFeeParams::new(250, 4),
            ),
            (
                BaseFeeActivation::BlockNumber(136_165_876),
                ConstantBaseFeeParams::new(250, 2),
            ),
        ]))
    });

/// OP Sepolia chain ID
pub const SEPOLIA_CHAIN_ID: u64 = 0xaa37dc;

pub(crate) static SEPOLIA_BASE_FEE_PARAMS: LazyLock<BaseFeeParams<OpSpecId>> =
    LazyLock::new(|| {
        BaseFeeParams::Dynamic(DynamicBaseFeeParams::new(vec![
            (
                BaseFeeActivation::Hardfork(OpSpecId::BEDROCK),
                ConstantBaseFeeParams::new(50, 6),
            ),
            (
                BaseFeeActivation::Hardfork(OpSpecId::CANYON),
                ConstantBaseFeeParams::new(250, 6),
            ),
            (
                BaseFeeActivation::BlockNumber(26_806_602),
                ConstantBaseFeeParams::new(250, 2),
            ),
        ]))
    });
