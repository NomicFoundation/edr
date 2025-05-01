use core::fmt::Debug;

pub use revm_context_interface::result::HaltReasonTr as HaltReasonTrait;

use crate::{
    block::Block,
    eips::eip1559::BaseFeeParams,
    l1,
    transaction::{ExecutableTransaction, TransactionValidation},
};

/// Trait for chain specifications.
pub trait ChainSpec {
    /// The chain's block type.
    type BlockEnv: Block;
    /// The chain's type for contextual information.
    type Context: Debug + Default;
    /// The chain's halt reason type.
    type HaltReason: HaltReasonTrait + 'static;
    /// The chain's hardfork type.
    type Hardfork: Copy + Into<l1::SpecId>;
    /// The chain's signed transaction type.
    type SignedTransaction: ExecutableTransaction
        + revm_context_interface::Transaction
        + TransactionValidation;
}

/// Constants for constructing Ethereum headers.
pub trait EthHeaderParams: ChainSpec<Hardfork: 'static + PartialOrd> {
    /// The minimum difficulty for the Ethash proof-of-work algorithm.
    const MIN_ETHASH_DIFFICULTY: u64;

    /// Parameters for the EIP-1559 base fee calculation.
    fn chain_base_fee_params(chain_id: u64) -> BaseFeeParams<Self::Hardfork>;
}
