//! Ethereum Virtual Machine (EVM) specification types

mod transaction;

use core::fmt::Debug;

use edr_eip1559::BaseFeeParams;
pub use revm_context_interface::{result::HaltReasonTr as HaltReasonTrait, Block};

pub use self::transaction::{ExecutableTransaction, TransactionValidation};

/// The identifier type for a specification used by the EVM.
pub type EvmSpecId = revm_primitives::hardfork::SpecId;

/// Trait for specifying the hardfork type of a chain.
pub trait ChainHardfork {
    /// The chain's hardfork type.
    type Hardfork: Copy + Into<EvmSpecId>;
}

/// Trait for chain specifications.
pub trait ChainSpec {
    /// The chain's block type.
    type BlockEnv: Block;
    /// The chain's type for contextual information.
    type Context: Debug + Default;
    /// The chain's halt reason type.
    type HaltReason: HaltReasonTrait + 'static;
    /// The chain's signed transaction type.
    type SignedTransaction: ExecutableTransaction
        + revm_context_interface::Transaction
        + TransactionValidation;
}

/// Constants for constructing Ethereum headers.
pub trait EthHeaderConstants: ChainHardfork<Hardfork: 'static + PartialOrd> {
    /// Parameters for the EIP-1559 base fee calculation.
    const BASE_FEE_PARAMS: BaseFeeParams<Self::Hardfork>;

    /// The minimum difficulty for the Ethash proof-of-work algorithm.
    const MIN_ETHASH_DIFFICULTY: u64;
}
