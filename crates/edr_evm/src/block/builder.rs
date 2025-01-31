mod l1;

use std::fmt::Debug;

use edr_eth::{
    block::{BlockOptions, PartialHeader},
    spec::ChainSpec,
    Address,
};
use revm_handler::FrameResult;
use revm_handler_interface::Frame;
use revm_interpreter::FrameInput;

pub use self::l1::{EthBlockBuilder, EthBlockReceiptFactory};
use crate::{
    blockchain::SyncBlockchain,
    config::CfgEnv,
    extension::ExtendedContext,
    spec::{ContextForChainSpec, RuntimeSpec},
    state::{DatabaseComponentError, DatabaseComponents, SyncState, WrapDatabaseRef},
    transaction::TransactionError,
    ContextExtension, MineBlockResultAndStateForChainSpec,
};

/// An error caused during construction of a block builder.
#[derive(Debug, thiserror::Error)]
pub enum BlockBuilderCreationError<BlockchainErrorT, HardforkT, StateErrorT> {
    /// Blockchain error
    #[error(transparent)]
    Blockchain(BlockchainErrorT),
    /// State error
    #[error(transparent)]
    State(StateErrorT),
    /// Unsupported hardfork. Hardforks older than Byzantium are not supported
    #[error("Unsupported hardfork: {0:?}. Hardforks older than Byzantium are not supported.")]
    UnsupportedHardfork(HardforkT),
}

/// Helper type for a chain-specific [`BlockBuilderCreationError`].
pub type BlockBuilderCreationErrorForChainSpec<BlockchainErrorT, ChainSpecT, StateErrorT> =
    BlockBuilderCreationError<BlockchainErrorT, <ChainSpecT as ChainSpec>::Hardfork, StateErrorT>;

impl<BlockchainErrorT, HardforkT: Debug, StateErrorT>
    From<DatabaseComponentError<BlockchainErrorT, StateErrorT>>
    for BlockBuilderCreationError<BlockchainErrorT, HardforkT, StateErrorT>
{
    fn from(value: DatabaseComponentError<BlockchainErrorT, StateErrorT>) -> Self {
        match value {
            DatabaseComponentError::Blockchain(error) => Self::Blockchain(error),
            DatabaseComponentError::State(error) => Self::State(error),
        }
    }
}

/// An error caused during execution of a transaction while building a block.
#[derive(Debug, thiserror::Error)]
pub enum BlockTransactionError<BlockchainErrorT, ChainSpecT, StateErrorT>
where
    ChainSpecT: ChainSpec,
{
    /// Transaction has higher gas limit than is remaining in block
    #[error("Transaction has a higher gas limit than the remaining gas in the block")]
    ExceedsBlockGasLimit,
    /// Transaction has higher blob gas usage than is remaining in block
    #[error("Transaction has higher blob gas usage than is remaining in block")]
    ExceedsBlockBlobGasLimit,
    /// Transaction error
    #[error(transparent)]
    Transaction(#[from] TransactionError<BlockchainErrorT, ChainSpecT, StateErrorT>),
}

/// A trait for building blocks.
pub trait BlockBuilder<'blockchain, ChainSpecT>: Sized
where
    ChainSpecT: RuntimeSpec,
{
    /// The blockchain's error type.
    type BlockchainError: std::error::Error;

    /// The state's error type.
    type StateError: Send + std::error::Error;

    /// Creates a new block builder.
    fn new_block_builder(
        blockchain: &'blockchain dyn SyncBlockchain<
            ChainSpecT,
            Self::BlockchainError,
            Self::StateError,
        >,
        state: Box<dyn SyncState<Self::StateError>>,
        cfg: CfgEnv<ChainSpecT::Hardfork>,
        options: BlockOptions,
    ) -> Result<
        Self,
        BlockBuilderCreationErrorForChainSpec<Self::BlockchainError, ChainSpecT, Self::StateError>,
    >;

    /// Returns the block's receipt factory.
    fn block_receipt_factory(&self) -> ChainSpecT::BlockReceiptFactory;

    /// Returns the block's [`PartialHeader`].
    fn header(&self) -> &PartialHeader;

    /// Adds a transaction to the block.
    fn add_transaction(
        &mut self,
        transaction: ChainSpecT::SignedTransaction,
    ) -> Result<(), BlockTransactionError<Self::BlockchainError, ChainSpecT, Self::StateError>>;

    /// Adds a transaction to the block.
    fn add_transaction_with_extension<'context, 'extension, ExtensionT, FrameT>(
        &mut self,
        transaction: ChainSpecT::SignedTransaction,
        extension: &'extension mut ContextExtension<ExtensionT, FrameT>,
    ) -> Result<(), BlockTransactionError<Self::BlockchainError, ChainSpecT, Self::StateError>>
    where
        'blockchain: 'context,
        'extension: 'context,
        ChainSpecT: 'context,
        FrameT: Frame<
            Context<'context> = ExtendedContext<
                'context,
                ContextForChainSpec<
                    ChainSpecT,
                    WrapDatabaseRef<
                        DatabaseComponents<
                            &'context dyn SyncBlockchain<
                                ChainSpecT,
                                Self::BlockchainError,
                                Self::StateError,
                            >,
                            &'context dyn SyncState<Self::StateError>,
                        >,
                    >,
                >,
                ExtensionT,
            >,
            Error = TransactionError<Self::BlockchainError, ChainSpecT, Self::StateError>,
            FrameInit = FrameInput,
            FrameResult = FrameResult,
        >,
        Self::BlockchainError: 'context,
        Self::StateError: 'context;

    /// Finalizes the block, applying rewards to the state.
    fn finalize(
        self,
        rewards: Vec<(Address, u128)>,
    ) -> Result<MineBlockResultAndStateForChainSpec<ChainSpecT, Self::StateError>, Self::StateError>;
}
