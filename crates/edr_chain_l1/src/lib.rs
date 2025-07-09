/// Types for L1's EIP-2718 envelope.
pub mod eip2718;
/// Ethereum L1 JSON-RPC types.
pub mod rpc;
mod spec;
/// L1 transaction types
pub mod transaction;

pub use edr_eth::EvmSpecId as Hardfork;
pub use revm_context::BlockEnv;
pub use revm_context_interface::result::{HaltReason, InvalidTransaction};

/// L1 chain type
pub const CHAIN_TYPE: &str = "L1";

pub type L1BlockEnv = BlockEnv;
pub type L1HaltReason = HaltReason;
pub type L1Hardfork = Hardfork;
pub type L1InvalidTransaction = InvalidTransaction;
