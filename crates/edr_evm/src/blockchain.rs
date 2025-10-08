use std::{collections::BTreeMap, fmt::Debug, ops::Bound::Included};

use edr_block_api::Block;
use edr_block_storage::ReservableSparseBlockStorage;
use edr_blockchain_api::{BlockHash, Blockchain, BlockchainMut};
use edr_evm_spec::{ChainHardfork, EvmSpecId, TransactionValidation};
use edr_primitives::B256;
use edr_receipt::ReceiptTrait;
use edr_state_api::{StateCommit, StateDiff, StateOverride};

// pub use self::{
//     forked::{CreationError as ForkedCreationError, ForkedBlockchain, ForkedBlockchainError},
//     local::{InvalidGenesisBlock, LocalBlockchain},
// };
use crate::{hardfork::Activations, spec::SyncRuntimeSpec};

/// Helper type for a chain-specific [`BlockchainError`].
pub type BlockchainErrorForChainSpec<ChainSpecT> = BlockchainError<
    <ChainSpecT as RuntimeSpec>::RpcBlockConversionError,
    <ChainSpecT as ChainHardfork>::Hardfork,
    <ChainSpecT as RuntimeSpec>::RpcReceiptConversionError,
>;

/// Combinatorial error for the blockchain API.
#[derive(Debug, thiserror::Error)]
pub enum BlockchainError<BlockConversionErrorT, HardforkT: Debug, ReceiptConversionErrorT> {
    /// Forked blockchain error
    #[error(transparent)]
    Forked(#[from] ForkedBlockchainError<BlockConversionErrorT, ReceiptConversionErrorT>),
    /// An error that occurs when trying to insert a block into storage.
    #[error(transparent)]
    Insert(#[from] edr_block_storage::InsertBlockError),
    /// Missing hardfork activation history
    #[error(
        "No known hardfork for execution on historical block {block_number} (relative to fork block number {fork_block_number}) in chain with id {chain_id}. The node was not configured with a hardfork activation history."
    )]
    MissingHardforkActivations {
        /// Block number
        block_number: u64,
        /// Fork block number
        fork_block_number: u64,
        /// Chain id
        chain_id: u64,
    },
    /// Block number does not exist in blockchain
    #[error("Unknown block number")]
    UnknownBlockNumber,
    /// No hardfork found for block
    #[error(
        "Could not find a hardfork to run for block {block_number}, after having looked for one in the hardfork activation history, which was: {hardfork_activations:?}."
    )]
    UnknownBlockSpec {
        /// Block number
        block_number: u64,
        /// Hardfork activation history
        hardfork_activations: Activations<HardforkT>,
    },
}

/// Trait that meets all requirements for a synchronous blockchain.
pub trait SyncBlockchain<
    BlockReceiptT: Send + Sync,
    BlockT: ?Sized,
    BlockchainErrorT: Debug + Send,
    HardforkT: Send + Sync,
    LocalBlockT: Send + Sync,
    SignedTransactionT: Send + Sync,
    StateErrorT,
>:
    Blockchain<
        BlockT,
        BlockReceiptT,
        HardforkT,
        BlockchainError = BlockchainErrorT,
        StateError = StateErrorT,
    > + BlockchainMut<BlockT, LocalBlockT, SignedTransactionT, Error = BlockchainErrorT>
    + BlockHash<Error = BlockchainErrorT>
    + Send
    + Sync
    + Debug
{
}

impl<
        BlockReceiptT: Send + Sync,
        BlockT: ?Sized,
        BlockchainErrorT: Debug + Send,
        BlockchainT: Blockchain<
                BlockT,
                BlockReceiptT,
                HardforkT,
                BlockchainError = BlockchainErrorT,
                StateError = StateErrorT,
            > + BlockchainMut<BlockT, LocalBlockT, SignedTransactionT, Error = BlockchainErrorT>
            + BlockHash<Error = BlockchainErrorT>
            + Send
            + Sync
            + Debug,
        HardforkT: Send + Sync,
        LocalBlockT: Send + Sync,
        SignedTransactionT: TransactionValidation<ValidationError: Send + Sync> + Send + Sync,
        StateErrorT,
    >
    SyncBlockchain<
        BlockReceiptT,
        BlockT,
        BlockchainErrorT,
        HardforkT,
        LocalBlockT,
        SignedTransactionT,
        StateErrorT,
    > for BlockchainT
{
}

/// Helper type for a chain-specific [`SyncBlockchain`].
pub trait SyncBlockchainForChainSpec<
    BlockchainErrorT: Debug + Send,
    ChainSpecT: SyncRuntimeSpec,
    StateErrorT,
>:
    SyncBlockchain<
    ChainSpecT::BlockReceipt,
    ChainSpecT::Block,
    BlockchainErrorT,
    ChainSpecT::Hardfork,
    ChainSpecT::LocalBlock,
    ChainSpecT::SignedTransaction,
    StateErrorT,
>
{
}

impl<
        BlockchainErrorT: Debug + Send,
        BlockchainT: SyncBlockchain<
            ChainSpecT::BlockReceipt,
            ChainSpecT::Block,
            BlockchainErrorT,
            ChainSpecT::Hardfork,
            ChainSpecT::LocalBlock,
            ChainSpecT::SignedTransaction,
            StateErrorT,
        >,
        ChainSpecT: SyncRuntimeSpec,
        StateErrorT,
    > SyncBlockchainForChainSpec<BlockchainErrorT, ChainSpecT, StateErrorT> for BlockchainT
{
}
