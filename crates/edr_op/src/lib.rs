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

pub use op_revm::L1BlockInfo;

pub use self::spec::OpChainSpec;

/// OP Stack chain type
pub const CHAIN_TYPE: &str = "op";

/// OP Stack halt reason.
pub type HaltReason = op_revm::OpHaltReason;

/// OP Stack hardfork.
pub type Hardfork = op_revm::OpSpecId;

/// OP Stack invalid header error.
pub type InvalidHeader = revm_context_interface::result::InvalidHeader;

/// OP Stack invalid transaction error.
pub type InvalidTransaction = op_revm::OpTransactionError;
