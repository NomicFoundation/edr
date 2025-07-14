#![warn(missing_docs)]

//! Ethereum L1 types
//!
//! L1 types as needed by EDR. They are based on the same primitive types as
//! `edr_eth`.

/// Types for L1's EIP-2718 envelope.
pub mod eip2718;
/// Types and constants for Ethereum L1 harforks.
pub mod hardfork;
/// Types for Ethereum L1 receipts.
pub mod receipt;
/// Ethereum L1 JSON-RPC types.
pub mod rpc;
mod spec;
/// L1 transaction types
pub mod transaction;

pub use edr_eth::block;

pub use self::spec::L1ChainSpec;

/// L1 chain type
pub const CHAIN_TYPE: &str = "L1";

/// L1 block environment type
pub type L1BlockEnv = edr_evm::BlockEnv;

/// Convenience type alias for [`L1BlockEnv`].
///
/// This allows usage like [`edr_chain_l1::BlockEnv`].
pub type BlockEnv = L1BlockEnv;

/// L1 halt reason type
pub type L1HaltReason = revm_context_interface::result::HaltReason;

/// Convenience type alias for [`L1HaltReason`].
///
/// This allows usage like [`edr_chain_l1::HaltReason`].
pub type HaltReason = L1HaltReason;

/// L1 hardfork type
pub type L1Hardfork = edr_eth::EvmSpecId;

/// Convenience type alias for [`L1Hardfork`].
///
/// This allows usage like [`edr_chain_l1::Hardfork`].
pub type Hardfork = L1Hardfork;

/// L1 invalid transaction type
pub type L1InvalidTransaction = edr_evm::EvmInvalidTransaction;

/// Convenience type alias for [`L1InvalidTransaction`].
///
/// This allows usage like [`edr_chain_l1::InvalidTransaction`].
pub type InvalidTransaction = L1InvalidTransaction;
