//! Ethereum block builder API

use std::fmt::Debug;

use edr_block_header::{BlockHeader, HeaderOverrides, PartialHeader, Withdrawal};
pub use edr_blockchain_api::Blockchain;
use edr_chain_spec::{
    BlockEnvChainSpec, ChainSpec, EvmSpecId, HaltReasonTrait, TransactionValidation,
};
pub use edr_database_components::{DatabaseComponentError, DatabaseComponents, WrapDatabaseRef};
use edr_evm_spec::{config::EvmConfig, ContextForChainSpec, EvmChainSpec};
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
    // As this generic type always needs to be specified, placing it second makes the function
    // easier to use; e.g.
    // ```
    // BlockBuilder::<'_, MyChainSpec, _, _>
    // ```
    ChainSpecT: ?Sized
        + BlockEnvChainSpec
        + EvmChainSpec,
    BlockReceiptT,
    BlockT: ?Sized,
>: Sized
{
    /// The blockchain's error type.
    type BlockchainError: std::error::Error;

    /// The local block type constructed by the builder.
    type LocalBlock;

    /// The state's error type.
    type StateError: std::error::Error;

    /// Creates a new block builder.
    fn new_block_builder(
        blockchain: &'builder dyn Blockchain<
            BlockReceiptT,
            BlockT,
            Self::BlockchainError,
            ChainSpecT::Hardfork,
            Self::LocalBlock,
            ChainSpecT::SignedTransaction,
            Self::StateError,
        >,
        state: Box<dyn SyncState<Self::StateError>>,
        evm_config: &EvmConfig,
        inputs: BlockInputs,
        overrides: HeaderOverrides<ChainSpecT::Hardfork>,
        custom_precompiles: &'builder HashMap<Address, PrecompileFn>,
    ) -> Result<
        Self,
        BlockBuilderCreationError<
            DatabaseComponentError<Self::BlockchainError, Self::StateError>,
            ChainSpecT::Hardfork,
        >,
    >;

    /// Returns the block's [`PartialHeader`].
    fn header(&self) -> &PartialHeader;

    /// Adds a transaction to the block.
    fn add_transaction(
        &mut self,
        transaction: ChainSpecT::SignedTransaction,
    ) -> Result<
        (),
        BlockTransactionError<
            DatabaseComponentError<Self::BlockchainError, Self::StateError>,
            <ChainSpecT::SignedTransaction as TransactionValidation>::ValidationError,
        >,
    >;

    /// Adds a transaction to the block.
    fn add_transaction_with_inspector<InspectorT>(
        &mut self,
        transaction: ChainSpecT::SignedTransaction,
        inspector: &mut InspectorT,
    ) -> Result<
        (),
        BlockTransactionError<
            DatabaseComponentError<Self::BlockchainError, Self::StateError>,
            <ChainSpecT::SignedTransaction as TransactionValidation>::ValidationError,
        >,
    >
    where
        InspectorT: for<'inspector> Inspector<
            ContextForChainSpec<
                ChainSpecT,
                ChainSpecT::BlockEnv<'inspector, PartialHeader>,
                WrapDatabaseRef<
                    DatabaseComponents<
                        &'inspector dyn Blockchain<
                            BlockReceiptT,
                            BlockT,
                            Self::BlockchainError,
                            ChainSpecT::Hardfork,
                            Self::LocalBlock,
                            ChainSpecT::SignedTransaction,
                            Self::StateError,
                        >,
                        &'inspector dyn SyncState<Self::StateError>,
                    >,
                >,
            >,
        >;

    /// Finalizes the block, applying rewards to the state.
    fn finalize_block(
        self,
        rewards: Vec<(Address, u128)>,
    ) -> Result<
        BuiltBlockAndState<ChainSpecT::HaltReason, Self::LocalBlock, Self::StateError>,
        Self::StateError,
    >;
}
