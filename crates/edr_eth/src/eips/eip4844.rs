pub use alloy_eips::eip4844::TARGET_BLOBS_PER_BLOCK_DENCUN as TARGET_BLOBS_PER_BLOCK;
pub use c_kzg::{ethereum_kzg_settings, KzgSettings};
use edr_evm_spec::EvmSpecId;
pub use revm_context_interface::block::{
    calc_blob_gasprice, calc_excess_blob_gas, BlobExcessGasAndPrice,
};
pub use revm_primitives::eip4844::{
    BLOB_BASE_FEE_UPDATE_FRACTION_CANCUN, BLOB_BASE_FEE_UPDATE_FRACTION_PRAGUE, GAS_PER_BLOB,
    MAX_BLOB_GAS_PER_BLOCK_CANCUN, TARGET_BLOB_GAS_PER_BLOCK_CANCUN, VERSIONED_HASH_VERSION_KZG,
};

/// Blob base fee update fraction per hard fork.
pub fn blob_base_fee_update_fraction(hardfork: EvmSpecId) -> u64 {
    if hardfork >= EvmSpecId::PRAGUE {
        BLOB_BASE_FEE_UPDATE_FRACTION_PRAGUE
    } else {
        BLOB_BASE_FEE_UPDATE_FRACTION_CANCUN
    }
}
