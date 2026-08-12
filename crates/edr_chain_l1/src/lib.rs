//! Ethereum L1 chain types

pub mod block;
pub mod chains;
pub mod difficulty;
mod eip2718;
pub mod hardfork;
pub mod pooled;
pub mod receipt;
pub mod request;
pub mod reward;
pub mod rpc;
pub mod signed;
mod spec;
pub mod r#type;

use edr_eip1559::{BaseFeeParams, ConstantBaseFeeParams};
pub use revm_context_interface::result::OutOfGasError;

pub use self::{
    eip2718::TypedEnvelope, hardfork::L1Hardfork, pooled::L1PooledTransaction,
    r#type::L1TransactionType, request::L1TransactionRequest, signed::L1SignedTransaction,
    spec::L1ChainSpec,
};

/// Ethereum L1 EVM-level hardfork; see
/// [`EvmHardforkChainSpec::EvmHardfork`](edr_chain_spec::EvmHardforkChainSpec::EvmHardfork).
pub type EvmHardfork = edr_chain_spec::EvmSpecId;

/// Ethereum L1 halt reason.
pub type HaltReason = revm_context_interface::result::HaltReason;

/// Convenience alias for the Ethereum L1 hardfork.
pub type Hardfork = L1Hardfork;

/// Ethereum L1 invalid header error.
pub type InvalidHeader = revm_context_interface::result::InvalidHeader;

/// Ethereum L1 invalid transaction error.
pub type InvalidTransaction = revm_context_interface::result::InvalidTransaction;

/// L1 Ethereum chain type
pub const CHAIN_TYPE: &str = "L1";

/// Base fee parameters for L1 Ethereum.
pub const L1_BASE_FEE_PARAMS: BaseFeeParams<Hardfork> =
    BaseFeeParams::Constant(ConstantBaseFeeParams::ethereum());

/// The minimum difficulty for the Ethash proof-of-work algorithm.
pub const L1_MIN_ETHASH_DIFFICULTY: u64 = 131_072;

/// Ethereum L1 extra data for genesis blocks.
pub const L1_GENESIS_BLOCK_EXTRA_DATA: &[u8] = b"\x12\x34";
