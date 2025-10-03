use std::sync::LazyLock;

use edr_eip1559::{BaseFeeActivation, BaseFeeParams, ConstantBaseFeeParams, DynamicBaseFeeParams};
use op_revm::OpSpecId;

/// Base Mainnet chain ID
pub const MAINNET_CHAIN_ID: u64 = 8453;

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
        ]))
    });

/// Base Sepolia chain ID
pub const SEPOLIA_CHAIN_ID: u64 = 84532;

pub(crate) static SEPOLIA_BASE_FEE_PARAMS: LazyLock<BaseFeeParams<OpSpecId>> =
    LazyLock::new(|| {
        BaseFeeParams::Dynamic(DynamicBaseFeeParams::new(vec![
            (
                BaseFeeActivation::Hardfork(OpSpecId::BEDROCK),
                ConstantBaseFeeParams::new(50, 10),
            ),
            (
                BaseFeeActivation::Hardfork(OpSpecId::CANYON),
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
