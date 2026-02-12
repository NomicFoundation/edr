//! Ethereum block builder API

use std::fmt::Debug;

use edr_block_header::{BlockConfig, BlockHeader, HeaderOverrides, PartialHeader, Withdrawal};
pub use edr_blockchain_api::Blockchain;
use edr_chain_spec::{
    BlockEnvChainSpec, ChainSpec, EvmSpecId, HaltReasonTrait, HardforkChainSpec,
    TransactionValidation,
};
use edr_chain_spec_evm::{config::EvmConfig, ContextForChainSpec, EvmChainSpec};
pub use edr_chain_spec_evm::{
    result::ExecutionResult, CfgEnv, Context, Inspector, Journal, TransactionError,
};
pub use edr_database_components::{DatabaseComponentError, DatabaseComponents, WrapDatabaseRef};
use edr_primitives::{Address, HashMap, HashSet};
use edr_state_api::{DynState, StateDiff, StateError};
pub use revm_precompile::PrecompileFn;

/// Helper type for a chain-specific [`BlockBuilderCreationError`].
pub type BlockBuilderCreationErrorForChainSpec<ChainSpecT, DatabaseErrorT> =
    BlockBuilderCreationError<DatabaseErrorT, <ChainSpecT as HardforkChainSpec>::Hardfork>;

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
    /// Constructs empty block inputs for the provided hardfork; i.e. no
    /// withdrawals nor ommers.
    pub fn empty<HardforkT: Into<EvmSpecId>>(hardfork: HardforkT) -> Self {
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

#[derive(Debug, thiserror::Error)]
pub enum BlockFinalizeError<StateErrorT> {
    /// Maximum block RLP size exceeded (EIP-7934).
    #[error(
        "Maximum block RLP size exceeded. Maximum: {max_size} bytes. Actual: {actual_size} bytes"
    )]
    BlockRlpSizeExceeded { max_size: usize, actual_size: usize },
    /// State error.
    #[error(transparent)]
    State(StateErrorT),
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
pub struct BuiltBlockAndState<BlockT, HaltReasonT: HaltReasonTrait> {
    /// Mined block
    pub block: BlockT,
    /// State after mining the block
    pub state: Box<dyn DynState>,
    /// State diff applied by block
    pub state_diff: StateDiff,
    /// Transaction results
    pub transaction_results: Vec<ExecutionResult<HaltReasonT>>,
}

pub struct BuiltBlockAndStateWithMetadata<BlockT, HaltReasonT: HaltReasonTrait> {
    /// Mined block and state
    pub block_and_state: BuiltBlockAndState<BlockT, HaltReasonT>,
    /// The set of precompile addresses that were available during execution.
    pub precompile_addresses: HashSet<Address>,
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

    /// Creates a new block builder.
    fn new_block_builder(
        blockchain: &'builder dyn Blockchain<
            BlockReceiptT,
            BlockT,
            Self::BlockchainError,
            ChainSpecT::Hardfork,
            Self::LocalBlock,
            ChainSpecT::SignedTransaction,
        >,
        block_config: &'builder BlockConfig<ChainSpecT::Hardfork>,
        state: Box<dyn DynState>,
        evm_config: &EvmConfig,
        inputs: BlockInputs,
        overrides: HeaderOverrides<ChainSpecT::Hardfork>,
        custom_precompiles: &'builder HashMap<Address, PrecompileFn>,
    ) -> Result<
        Self,
        BlockBuilderCreationErrorForChainSpec<
            ChainSpecT,
            DatabaseComponentError<Self::BlockchainError, StateError>,
        >,
    >;

    /// Returns the block's [`PartialHeader`].
    fn header(&self) -> &PartialHeader;

    /// Returns the set of precompile addresses available during execution.
    fn precompile_addresses(&self) -> &HashSet<Address>;

    /// Adds a transaction to the block.
    fn add_transaction(
        &mut self,
        transaction: ChainSpecT::SignedTransaction,
    ) -> Result<
        (),
        BlockTransactionErrorForChainSpec<
            ChainSpecT,
            DatabaseComponentError<Self::BlockchainError, StateError>,
        >,
    >;

    /// Adds a transaction to the block.
    fn add_transaction_with_inspector<InspectorT>(
        &mut self,
        transaction: ChainSpecT::SignedTransaction,
        inspector: &mut InspectorT,
    ) -> Result<
        (),
        BlockTransactionErrorForChainSpec<
            ChainSpecT,
            DatabaseComponentError<Self::BlockchainError, StateError>,
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
                        >,
                        &'inspector dyn DynState,
                    >,
                >,
            >,
        >;

    /// Finalizes the block, applying rewards to the state.
    fn finalize_block(
        self,
        rewards: Vec<(Address, u128)>,
    ) -> Result<
        BuiltBlockAndStateWithMetadata<Self::LocalBlock, ChainSpecT::HaltReason>,
        BlockFinalizeError<StateError>,
    >;
}
