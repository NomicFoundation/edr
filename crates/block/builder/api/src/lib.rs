//! Ethereum block builder API

use std::fmt::Debug;

use edr_block_header::{
    BlockConfig, BlockHeader, HeaderAndEvmSpec, HeaderOverrides, PartialHeader, Withdrawal,
};
pub use edr_blockchain_api::sync::SyncBlockchain;
use edr_chain_spec::{ChainSpec, EvmSpecId, HaltReasonTrait, TransactionValidation};
pub use edr_database_components::{DatabaseComponentError, DatabaseComponents, WrapDatabaseRef};
use edr_evm_spec::{config::EvmConfig, EvmChainSpec};
pub use edr_evm_spec::{
    result::ExecutionResult, CfgEnv, Context, Inspector, Journal, TransactionError,
};
use edr_primitives::{Address, HashMap};
use edr_state_api::{StateDiff, SyncState};
pub use revm_precompile::PrecompileFn;

/// An error caused during construction of a block builder.
#[derive(Debug, thiserror::Error)]
pub enum BlockBuilderCreationError<DatabaseErrorT, HardforkT> {
    /// Database error
    #[error(transparent)]
    Database(DatabaseErrorT),
    /// Missing withdrawals. The chain expects withdrawals to be present
    /// post-Shanghai hardfork.
    #[error(
        "Missing withdrawals. The chain expects withdrawals to be present post-Shanghai hardfork."
    )]
    MissingWithdrawals,
    /// Unsupported hardfork. Hardforks older than Byzantium are not supported
    #[error("Unsupported hardfork: {0:?}. Hardforks older than Byzantium are not supported.")]
    UnsupportedHardfork(HardforkT),
}

/// Chain-agnostic inputs for building a block.
#[derive(Debug, Default)]
pub struct BlockInputs {
    /// The ommers of the block.
    pub ommers: Vec<BlockHeader>,
    /// The withdrawals of the block. Present post-Shanghai hardfork.
    pub withdrawals: Option<Vec<Withdrawal>>,
}

impl BlockInputs {
    // TODO: https://github.com/NomicFoundation/edr/issues/990
    // Add support for specifying withdrawals
    /// Constructs default block inputs for the provided hardfork.
    pub fn new<HardforkT: Into<EvmSpecId>>(hardfork: HardforkT) -> Self {
        let withdrawals = if hardfork.into() >= EvmSpecId::SHANGHAI {
            Some(Vec::new())
        } else {
            None
        };

        Self {
            ommers: Vec::new(),
            withdrawals,
        }
    }
}

/// Helper type for a chain-specific [`BlockTransactionError`].
pub type BlockTransactionErrorForChainSpec<ChainSpecT, DatabaseErrorT> = BlockTransactionError<
    DatabaseErrorT,
    <<ChainSpecT as ChainSpec>::SignedTransaction as TransactionValidation>::ValidationError,
>;

/// An error caused during execution of a transaction while building a block.
#[derive(Debug, thiserror::Error)]
pub enum BlockTransactionError<DatabaseErrorT, TransactionValidationErrorT> {
    /// Transaction has higher gas limit than is remaining in block
    #[error("Transaction has a higher gas limit than the remaining gas in the block")]
    ExceedsBlockGasLimit,
    /// Transaction has higher blob gas usage than is remaining in block
    #[error("Transaction has higher blob gas usage than is remaining in block")]
    ExceedsBlockBlobGasLimit,
    /// Transaction error
    #[error(transparent)]
    Transaction(#[from] TransactionError<DatabaseErrorT, TransactionValidationErrorT>),
}

/// The result of building a block, including the state. This result needs to be
/// inserted into the blockchain to be persistent.
#[derive(Debug)]
pub struct BuiltBlockAndState<HaltReasonT: HaltReasonTrait, LocalBlockT, StateErrorT> {
    /// Mined block
    pub block: LocalBlockT,
    /// State after mining the block
    pub state: Box<dyn SyncState<StateErrorT>>,
    /// State diff applied by block
    pub state_diff: StateDiff,
    /// Transaction results
    pub transaction_results: Vec<ExecutionResult<HaltReasonT>>,
}

/// A trait for building blocks.
pub trait BlockBuilder<
    'builder,
    BlockReceiptT: Send + Sync,
    BlockT: ?Sized,
    EvmChainSpecT: ?Sized
        + EvmChainSpec<Hardfork: Send + Sync, SignedTransaction: TransactionValidation + Send + Sync>,
>: Sized
{
    /// The blockchain's error type.
    type BlockchainError: std::error::Error;

    /// The local block type constructed by the builder.
    type LocalBlock: Send + Sync;

    /// The state's error type.
    type StateError: Send + std::error::Error;

    /// Creates a new block builder.
    fn new_block_builder(
        blockchain: &'builder dyn SyncBlockchain<
            BlockReceiptT,
            BlockT,
            Self::BlockchainError,
            EvmChainSpecT::Hardfork,
            Self::LocalBlock,
            EvmChainSpecT::SignedTransaction,
            Self::StateError,
        >,
        state: Box<dyn SyncState<Self::StateError>>,
        block_config: BlockConfig<'_, EvmChainSpecT::Hardfork>,
        evm_config: EvmConfig,
        inputs: BlockInputs,
        overrides: HeaderOverrides<EvmChainSpecT::Hardfork>,
        custom_precompiles: &'builder HashMap<Address, PrecompileFn>,
    ) -> Result<
        Self,
        BlockBuilderCreationError<
            DatabaseComponentError<Self::BlockchainError, Self::StateError>,
            EvmChainSpecT::Hardfork,
        >,
    >;

    /// Returns the block's [`PartialHeader`].
    fn header(&self) -> &PartialHeader;

    /// Adds a transaction to the block.
    fn add_transaction(
        &mut self,
        transaction: EvmChainSpecT::SignedTransaction,
    ) -> Result<
        (),
        BlockTransactionError<
            DatabaseComponentError<Self::BlockchainError, Self::StateError>,
            <EvmChainSpecT::SignedTransaction as TransactionValidation>::ValidationError,
        >,
    >;

    /// Adds a transaction to the block.
    fn add_transaction_with_inspector<InspectorT>(
        &mut self,
        transaction: EvmChainSpecT::SignedTransaction,
        inspector: &mut InspectorT,
    ) -> Result<
        (),
        BlockTransactionError<
            DatabaseComponentError<Self::BlockchainError, Self::StateError>,
            <EvmChainSpecT::SignedTransaction as TransactionValidation>::ValidationError,
        >,
    >
    where
        InspectorT: for<'inspector> Inspector<
            Context<
                HeaderAndEvmSpec<'inspector, PartialHeader>,
                EvmChainSpecT::SignedTransaction,
                CfgEnv<EvmChainSpecT::Hardfork>,
                WrapDatabaseRef<
                    DatabaseComponents<
                        &'inspector dyn SyncBlockchain<
                            BlockReceiptT,
                            BlockT,
                            Self::BlockchainError,
                            EvmChainSpecT::Hardfork,
                            Self::LocalBlock,
                            EvmChainSpecT::SignedTransaction,
                            Self::StateError,
                        >,
                        &'inspector dyn SyncState<Self::StateError>,
                    >,
                >,
                Journal<
                    WrapDatabaseRef<
                        DatabaseComponents<
                            &'inspector dyn SyncBlockchain<
                                BlockReceiptT,
                                BlockT,
                                Self::BlockchainError,
                                EvmChainSpecT::Hardfork,
                                Self::LocalBlock,
                                EvmChainSpecT::SignedTransaction,
                                Self::StateError,
                            >,
                            &'inspector dyn SyncState<Self::StateError>,
                        >,
                    >,
                >,
                EvmChainSpecT::Context,
            >,
        >;

    /// Finalizes the block, applying rewards to the state.
    fn finalize(
        self,
        rewards: Vec<(Address, u128)>,
    ) -> Result<
        BuiltBlockAndState<EvmChainSpecT::HaltReason, Self::LocalBlock, Self::StateError>,
        Self::StateError,
    >;
}
