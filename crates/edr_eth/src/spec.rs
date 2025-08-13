use core::fmt::Debug;

pub use revm_context_interface::result::HaltReasonTr as HaltReasonTrait;
use serde::Deserialize;

use crate::{
    block::Block,
    eips::eip1559::BaseFeeParams,
    l1,
    transaction::{ExecutableTransaction, TransactionValidation},
};

/// Trait for specifying the hardfork type of a chain.
pub trait ChainHardfork {
    /// The chain's hardfork type.
    type Hardfork: Copy + Into<l1::SpecId>;
}
/// Trait for specifying the configuration specific to the chain type
pub trait ChainConfiguration {
    /// The chain's hardfork type.
    type Configuration: Copy + for<'a> Deserialize<'a> + Send;
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
