mod l1;

use std::fmt::Debug;

use edr_eth::{
    block::{BlockOptions, PartialHeader},
    spec::ChainSpec,
    Address, U256,
};

pub use self::l1::EthBlockBuilder;
use crate::{
    blockchain::SyncBlockchain,
    config::CfgEnv,
    spec::RuntimeSpec,
    state::{DatabaseComponentError, SyncState},
    transaction::TransactionError,
    DebugContext, MineBlockResultAndState,
};

/// An error caused during construction of a block builder.
#[derive(Debug, thiserror::Error)]
pub enum BlockBuilderCreationError<BlockchainErrorT, HardforkT: Debug, StateErrorT> {
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
pub enum BlockTransactionError<ChainSpecT, BlockchainErrorT, StateErrorT>
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
    Transaction(#[from] TransactionError<ChainSpecT, BlockchainErrorT, StateErrorT>),
}

/// A wrapper around a block builder and an error.
pub struct BlockBuilderAndError<BlockBuilderT, ErrorT> {
    pub block_builder: BlockBuilderT,
    pub error: ErrorT,
}

pub trait BlockBuilder<'blockchain, ChainSpecT, DebugDataT>: Sized
where
    ChainSpecT: RuntimeSpec,
{
    /// The blockchain's error type.
    type BlockchainError;

    /// The state's error type.
    type StateError: Debug + Send;

    /// Creates a new block builder.
    fn new_block_builder(
        blockchain: &'blockchain dyn SyncBlockchain<
            ChainSpecT,
            Self::BlockchainError,
            Self::StateError,
        >,
        state: Box<dyn SyncState<Self::StateError>>,
        hardfork: ChainSpecT::Hardfork,
        cfg: CfgEnv,
        options: BlockOptions,
        debug_context: Option<
            DebugContext<
                'blockchain,
                ChainSpecT,
                Self::BlockchainError,
                DebugDataT,
                Box<dyn SyncState<Self::StateError>>,
            >,
        >,
    ) -> Result<
        Self,
        BlockBuilderCreationError<Self::BlockchainError, ChainSpecT::Hardfork, Self::StateError>,
    >;

    /// Returns the block's [`PartialHeader`].
    fn header(&self) -> &PartialHeader;

    /// Adds a transaction to the block.
    fn add_transaction(
        self,
        transaction: ChainSpecT::SignedTransaction,
    ) -> Result<
        Self,
        BlockBuilderAndError<
            Self,
            BlockTransactionError<ChainSpecT, Self::BlockchainError, Self::StateError>,
        >,
    >;

    /// Finalizes the block, applying rewards to the state.
    fn finalize(
        self,
        rewards: Vec<(Address, U256)>,
    ) -> Result<
        MineBlockResultAndState<ChainSpecT::HaltReason, ChainSpecT::LocalBlock, Self::StateError>,
        Self::StateError,
    >;
}
