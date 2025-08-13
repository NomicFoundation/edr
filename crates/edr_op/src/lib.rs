#![warn(missing_docs)]

//! OP types
//!
//! OP types as needed by EDR. They are based on the same primitive types
//! as `revm`.

/// Types for OP blocks.
pub mod block;
/// Types for OP's EIP-1559 base fee parameters.
pub mod eip1559;
/// Types for OP's EIP-2718 envelope.
pub mod eip2718;
/// OP harforks.
pub mod hardfork;
/// Types and constants for OP predeploys.
pub mod predeploys;
/// Types for OP receipts.
pub mod receipt;
/// OP RPC types
pub mod rpc;
/// Types for running Solidity tests.
pub mod solidity_tests;
mod spec;
/// Utility types for testing.
#[cfg(any(test, feature = "test-utils"))]
pub mod test_utils;
/// OP transaction types
pub mod transaction;

use edr_eth::U256;
pub use op_revm::{OpHaltReason, OpSpecId};

pub use self::spec::{OpChainConfig, OpChainSpec};

/// OP Stack chain type
pub const CHAIN_TYPE: &str = "op";

/// Helper type for constructing an [`op_revm::L1BlockInfo`].
///
/// This type duplicates [`op_revm::L1BlockInfo`] but excludes the private
/// field to allow manual construction.
#[derive(Clone, Debug, Default)]
pub struct L1BlockInfo {
    /// The base fee of the L1 origin block.
    pub l1_base_fee: U256,
    /// The current L1 fee overhead. None if Ecotone is activated.
    pub l1_fee_overhead: Option<U256>,
    /// The current L1 fee scalar.
    pub l1_base_fee_scalar: U256,
    /// The current L1 blob base fee. None if Ecotone is not activated, except
    /// if `empty_scalars` is `true`.
    pub l1_blob_base_fee: Option<U256>,
    /// The current L1 blob base fee scalar. None if Ecotone is not activated.
    pub l1_blob_base_fee_scalar: Option<U256>,
}

impl From<L1BlockInfo> for op_revm::L1BlockInfo {
    fn from(value: L1BlockInfo) -> Self {
        // [`op_revm::L1BlockInfo`] contains a private field, so we need to construct it
        // like this
        let mut l1_block_info = Self::default();
        l1_block_info.l1_base_fee = value.l1_base_fee;
        l1_block_info.l1_fee_overhead = value.l1_fee_overhead;
        l1_block_info.l1_base_fee_scalar = value.l1_base_fee_scalar;
        l1_block_info.l1_blob_base_fee = value.l1_blob_base_fee;
        l1_block_info.l1_blob_base_fee_scalar = value.l1_blob_base_fee_scalar;

        l1_block_info
    }
}
