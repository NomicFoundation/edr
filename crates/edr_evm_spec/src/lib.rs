//! Ethereum Virtual Machine (EVM) specification types

mod transaction;

use core::fmt::Debug;

pub use revm_context_interface::{
    block::BlobExcessGasAndPrice,
    result::{HaltReasonTr as HaltReasonTrait, OutOfGasError},
    Block, Transaction,
};

pub use self::transaction::{ExecutableTransaction, TransactionValidation};

/// Halt reason type for the EVM.
pub type EvmHaltReason = revm_context_interface::result::HaltReason;

/// Error type for Ethereum header validation.
pub type EvmHeaderValidationError = revm_context_interface::result::InvalidHeader;

/// The identifier type for a specification used by the EVM.
pub type EvmSpecId = revm_primitives::hardfork::SpecId;

/// Error type for Ethereum transaction validation.
pub type EvmTransactionValidationError = revm_context_interface::result::InvalidTransaction;

/// Trait for specifying the hardfork type of a chain.
pub trait ChainHardfork {
    /// The chain's hardfork type.
    type Hardfork: Copy + Default + Into<EvmSpecId>;
}

/// Trait for chain specifications.
pub trait ChainSpec {
    /// The chain's block type.
    type BlockEnv: Block;
    /// The chain's type for contextual information.
    type Context: Debug;
    /// The chain's halt reason type.
    type HaltReason: HaltReasonTrait + 'static;
    /// The chain's signed transaction type.
    type SignedTransaction: ExecutableTransaction
        + revm_context_interface::Transaction
        + TransactionValidation;
}
