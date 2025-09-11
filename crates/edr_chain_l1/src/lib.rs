//! Ethereum L1 chain types

mod eip2718;
pub mod hardfork;
pub mod pooled;
pub mod request;
pub mod rpc;
pub mod signed;
mod spec;
pub mod r#type;

use edr_evm_spec::EvmSpecId;
pub use revm_context::TxEnv;
pub use revm_context_interface::result::OutOfGasError;
pub use revm_primitives::{
    alloy_primitives::Bloom,
    hardfork::UnknownHardfork,
    ruint::aliases::{B256, B64},
    Address, Bytes, U256,
};

pub use self::{
    eip2718::TypedEnvelope, pooled::PooledTransaction, r#type::Type, request::Request,
    signed::Signed, spec::L1ChainSpec,
};

/// Ethereum L1 block environment.
pub type BlockEnv = revm_context::BlockEnv;

/// Ethereum L1 halt reason.
pub type HaltReason = revm_context_interface::result::HaltReason;

/// Ethereum L1 hardfork.
pub type Hardfork = EvmSpecId;

/// Ethereum L1 invalid header error.
pub type InvalidHeader = revm_context_interface::result::InvalidHeader;

/// Ethereum L1 invalid transaction error.
pub type InvalidTransaction = revm_context_interface::result::InvalidTransaction;

/// L1 Ethereum chain type
pub const CHAIN_TYPE: &str = "L1";
