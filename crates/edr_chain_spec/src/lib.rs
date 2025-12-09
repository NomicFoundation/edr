//! Ethereum Virtual Machine (EVM) specification types

mod transaction;

use edr_eip7892::ScheduledBlobParams;
use edr_primitives::{Address, B256, U256};
pub use revm_context_interface::{
    block::BlobExcessGasAndPrice,
    result::{HaltReasonTr as HaltReasonTrait, OutOfGasError},
    Block as BlockEnvTrait, Transaction,
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

/// Trait for specifying the types representing a chain's block environment.
pub trait BlockEnvChainSpec: HardforkChainSpec {
    /// Type representing a block environment; i.e. the header of the block
    /// (being mined) and its hardfork.
    type BlockEnv<'header, BlockHeaderT>: BlockEnvConstructor<Self::Hardfork, &'header BlockHeaderT>
        + BlockEnvTrait
    where
        BlockHeaderT: 'header + BlockEnvForHardfork<Self::Hardfork>;
}

/// A trait for constructing a (partial) block header into an EVM block.
pub trait BlockEnvConstructor<HardforkT, HeaderT> {
    /// Converts the instance into an EVM block.
    fn new_block_env(header: HeaderT, hardfork: HardforkT, scheduled_blob_params: Option<ScheduledBlobParams>) -> Self;
}

/// Trait for providing block environment values for a specific hardfork.
pub trait BlockEnvForHardfork<HardforkT> {
    /// The number of ancestor blocks of this block (block height).
    fn number_for_hardfork(&self, hardfork: HardforkT) -> U256;

    /// Beneficiary (Coinbase, miner) is a address that have signed the block.
    ///
    /// This is the receiver address of priority gas rewards.
    fn beneficiary_for_hardfork(&self, hardfork: HardforkT) -> Address;

    /// The timestamp of the block in seconds since the UNIX epoch.
    fn timestamp_for_hardfork(&self, hardfork: HardforkT) -> U256;

    /// The gas limit of the block.
    fn gas_limit_for_hardfork(&self, hardfork: HardforkT) -> u64;

    /// The base fee per gas, added in the London upgrade with [EIP-1559].
    ///
    /// [EIP-1559]: https://eips.ethereum.org/EIPS/eip-1559
    fn basefee_for_hardfork(&self, hardfork: HardforkT) -> u64;

    /// The difficulty of the block.
    ///
    /// Unused after the Paris (AKA the merge) upgrade, and replaced by
    /// `prevrandao`.
    fn difficulty_for_hardfork(&self, hardfork: HardforkT) -> U256;

    /// The output of the randomness beacon provided by the beacon chain.
    ///
    /// Replaces `difficulty` after the Paris (AKA the merge) upgrade with
    /// [EIP-4399].
    ///
    /// Note: `prevrandao` can be found in a block in place of `mix_hash`.
    ///
    /// [EIP-4399]: https://eips.ethereum.org/EIPS/eip-4399
    fn prevrandao_for_hardfork(&self, hardfork: HardforkT) -> Option<B256>;

    /// Excess blob gas and blob gasprice.
    ///
    /// Incorporated as part of the Cancun upgrade via [EIP-4844].
    ///
    /// [EIP-4844]: https://eips.ethereum.org/EIPS/eip-4844
    fn blob_excess_gas_and_price_for_hardfork(
        &self,
        hardfork: HardforkT,
        scheduled_blob_params: Option<&ScheduledBlobParams>,
    ) -> Option<BlobExcessGasAndPrice>;

}

/// Trait for specifying the contextual information type of a chain.
pub trait ContextChainSpec {
    /// The chain's contextual information type.
    type Context;
}

/// Trait for specifying the hardfork type of a chain.
pub trait HardforkChainSpec {
    /// The chain's hardfork type.
    type Hardfork: Copy + Default + Into<EvmSpecId>;
}

/// Trait for chain specifications.
pub trait ChainSpec {
    /// The chain's halt reason type.
    type HaltReason: HaltReasonTrait + 'static;
    /// The chain's signed transaction type.
    type SignedTransaction: ExecutableTransaction
        + revm_context_interface::Transaction
        + TransactionValidation;
}
