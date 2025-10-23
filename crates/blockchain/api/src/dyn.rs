//! Types and traits for dynamic trait objects that implement blockchain
//! functionalities.

use core::fmt::{Debug, Display};
use std::{collections::BTreeMap, sync::Arc};

use edr_block_api::BlockAndTotalDifficulty;
use edr_primitives::{Address, HashSet, B256, U256};
use edr_receipt::log::FilterLog;
use edr_state_api::{DynState, StateDiff, StateOverride};

use crate::{
    BlockHashByNumber, BlockchainMetadata, BlockchainMetadataAtBlockNumber, GetBlockchainBlock,
    GetBlockchainLogs, InsertBlock, ReceiptByTransactionHash, ReserveBlocks, RevertToBlock,
    StateAtBlock, TotalDifficultyByBlockHash,
};

/// Wrapper around `DynBlockchainError`to allow implementation of
/// `std::error::Error`.
// This is required because of:
// <https://stackoverflow.com/questions/65151237/why-doesnt-boxdyn-error-implement-error#65151318>
pub struct DynBlockchainError {
    inner: Box<dyn std::error::Error>,
}

impl DynBlockchainError {
    fn new<ErrorT: 'static + std::error::Error>(error: ErrorT) -> Self {
        Self {
            inner: Box::new(error),
        }
    }
}

impl Debug for DynBlockchainError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        Debug::fmt(&self.inner, f)
    }
}

impl Display for DynBlockchainError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        Display::fmt(&self.inner, f)
    }
}

impl std::error::Error for DynBlockchainError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        self.inner.source()
    }

    fn description(&self) -> &str {
        // Deprecated method, but still need to forward it.
        #[allow(deprecated)]
        self.inner.description()
    }

    fn cause(&self) -> Option<&dyn std::error::Error> {
        // Deprecated method, but still need to forward it.
        #[allow(deprecated)]
        self.inner.cause()
    }
}

/// Trait for dynamic trait objects that implement [`BlockHashByNumber`].
pub trait DynBlockHashByNumber {
    /// Retrieves the block hash at the provided number.
    fn block_hash_by_number(&self, block_number: u64) -> Result<B256, DynBlockchainError>;
}

impl<T: BlockHashByNumber<Error: 'static + std::error::Error>> DynBlockHashByNumber for T {
    fn block_hash_by_number(&self, block_number: u64) -> Result<B256, DynBlockchainError> {
        self.block_hash_by_number(block_number)
            .map_err(DynBlockchainError::new)
    }
}

impl BlockHashByNumber for dyn DynBlockHashByNumber {
    type Error = DynBlockchainError;

    fn block_hash_by_number(&self, block_number: u64) -> Result<B256, Self::Error> {
        DynBlockHashByNumber::block_hash_by_number(self, block_number)
    }
}

/// Trait for dynamic trait objects that implement [`BlockchainMetadata`].
pub trait DynBlockchainMetadataAtBlockNumber<HardforkT> {
    /// Retrieves the chain ID of the block at the provided number.
    /// The chain ID can be different in fork mode pre- and post-fork block
    /// number.
    fn chain_id_at_block_number(&self, _block_number: u64) -> Result<u64, DynBlockchainError>;

    /// Retrieves the hardfork specification of the block at the provided
    /// number.
    fn spec_at_block_number(&self, block_number: u64) -> Result<HardforkT, DynBlockchainError>;
}

impl<
        T: BlockchainMetadataAtBlockNumber<HardforkT, Error: 'static + std::error::Error>,
        HardforkT,
    > DynBlockchainMetadataAtBlockNumber<HardforkT> for T
{
    fn chain_id_at_block_number(&self, block_number: u64) -> Result<u64, DynBlockchainError> {
        self.chain_id_at_block_number(block_number)
            .map_err(DynBlockchainError::new)
    }

    fn spec_at_block_number(&self, block_number: u64) -> Result<HardforkT, DynBlockchainError> {
        self.spec_at_block_number(block_number)
            .map_err(DynBlockchainError::new)
    }
}

/// Trait for dynamic trait objects that implement [`GetBlockchainBlock`].
pub trait DynGetBlockchainBlock<BlockT: ?Sized, HardforkT> {
    /// Retrieves the block with the provided hash, if it exists.
    #[allow(clippy::type_complexity)]
    fn block_by_hash(&self, hash: &B256) -> Result<Option<Arc<BlockT>>, DynBlockchainError>;

    /// Retrieves the block with the provided number, if it exists.
    #[allow(clippy::type_complexity)]
    fn block_by_number(&self, number: u64) -> Result<Option<Arc<BlockT>>, DynBlockchainError>;

    /// Retrieves the block that contains a transaction with the provided hash,
    /// if it exists.
    #[allow(clippy::type_complexity)]
    fn block_by_transaction_hash(
        &self,
        transaction_hash: &B256,
    ) -> Result<Option<Arc<BlockT>>, DynBlockchainError>;

    /// Retrieves the last block in the blockchain.
    fn last_block(&self) -> Result<Arc<BlockT>, DynBlockchainError>;
}

impl<
        T: GetBlockchainBlock<BlockT, HardforkT, Error: 'static + std::error::Error>,
        BlockT: ?Sized,
        HardforkT,
    > DynGetBlockchainBlock<BlockT, HardforkT> for T
{
    fn block_by_hash(&self, hash: &B256) -> Result<Option<Arc<BlockT>>, DynBlockchainError> {
        self.block_by_hash(hash).map_err(DynBlockchainError::new)
    }

    fn block_by_number(&self, number: u64) -> Result<Option<Arc<BlockT>>, DynBlockchainError> {
        self.block_by_number(number)
            .map_err(DynBlockchainError::new)
    }

    fn block_by_transaction_hash(
        &self,
        transaction_hash: &B256,
    ) -> Result<Option<Arc<BlockT>>, DynBlockchainError> {
        self.block_by_transaction_hash(transaction_hash)
            .map_err(DynBlockchainError::new)
    }

    fn last_block(&self) -> Result<Arc<BlockT>, DynBlockchainError> {
        self.last_block().map_err(DynBlockchainError::new)
    }
}

/// Trait for dynamic trait objects that implement [`GetBlockchainLogs`].
pub trait DynGetBlockchainLogs {
    /// Retrieves the logs that match the provided filter.
    fn logs(
        &self,
        from_block: u64,
        to_block: u64,
        addresses: &HashSet<Address>,
        normalized_topics: &[Option<Vec<B256>>],
    ) -> Result<Vec<FilterLog>, DynBlockchainError>;
}

impl<T: GetBlockchainLogs<Error: 'static + std::error::Error>> DynGetBlockchainLogs for T {
    fn logs(
        &self,
        from_block: u64,
        to_block: u64,
        addresses: &HashSet<Address>,
        normalized_topics: &[Option<Vec<B256>>],
    ) -> Result<Vec<FilterLog>, DynBlockchainError> {
        self.logs(from_block, to_block, addresses, normalized_topics)
            .map_err(DynBlockchainError::new)
    }
}

/// Trait for dynamic trait objects that implement [`InsertBlock`].
pub trait DynInsertBlock<BlockT: ?Sized, LocalBlockT, SignedTransactionT> {
    /// Inserts the provided block into the blockchain, returning a reference to
    /// the inserted block.
    fn insert_block(
        &mut self,
        block: LocalBlockT,
        state_diff: StateDiff,
    ) -> Result<BlockAndTotalDifficulty<Arc<BlockT>, SignedTransactionT>, DynBlockchainError>;
}

impl<
        T: InsertBlock<BlockT, LocalBlockT, SignedTransactionT, Error: 'static + std::error::Error>,
        BlockT: ?Sized,
        LocalBlockT,
        SignedTransactionT,
    > DynInsertBlock<BlockT, LocalBlockT, SignedTransactionT> for T
{
    fn insert_block(
        &mut self,
        block: LocalBlockT,
        state_diff: StateDiff,
    ) -> Result<BlockAndTotalDifficulty<Arc<BlockT>, SignedTransactionT>, DynBlockchainError> {
        self.insert_block(block, state_diff)
            .map_err(DynBlockchainError::new)
    }
}

/// Trait for dynamic trait objects that implement [`ReceiptByTransactionHash`].
pub trait DynReceiptByTransactionHash<BlockReceiptT> {
    /// Retrieves the receipt of the transaction with the provided hash, if it
    /// exists.
    fn receipt_by_transaction_hash(
        &self,
        transaction_hash: &B256,
    ) -> Result<Option<Arc<BlockReceiptT>>, DynBlockchainError>;
}

impl<
        T: ReceiptByTransactionHash<BlockReceiptT, Error: 'static + std::error::Error>,
        BlockReceiptT,
    > DynReceiptByTransactionHash<BlockReceiptT> for T
{
    fn receipt_by_transaction_hash(
        &self,
        transaction_hash: &B256,
    ) -> Result<Option<Arc<BlockReceiptT>>, DynBlockchainError> {
        self.receipt_by_transaction_hash(transaction_hash)
            .map_err(DynBlockchainError::new)
    }
}

/// Trait for dynamic trait objects that implement [`ReserveBlocks`].
pub trait DynReserveBlocks {
    /// Reserves the provided number of blocks, starting from the next block
    /// number.
    fn reserve_blocks(&mut self, additional: u64, interval: u64) -> Result<(), DynBlockchainError>;
}

impl<T: ReserveBlocks<Error: 'static + std::error::Error>> DynReserveBlocks for T {
    fn reserve_blocks(&mut self, additional: u64, interval: u64) -> Result<(), DynBlockchainError> {
        self.reserve_blocks(additional, interval)
            .map_err(DynBlockchainError::new)
    }
}

/// Trait for dynamic trait objects that implement [`RevertToBlock`].
pub trait DynRevertToBlock {
    /// Reverts to the block with the provided number, deleting all later
    /// blocks.
    fn revert_to_block(&mut self, block_number: u64) -> Result<(), DynBlockchainError>;
}

impl<T: RevertToBlock<Error: 'static + std::error::Error>> DynRevertToBlock for T {
    fn revert_to_block(&mut self, block_number: u64) -> Result<(), DynBlockchainError> {
        self.revert_to_block(block_number)
            .map_err(DynBlockchainError::new)
    }
}

/// Trait for dynamic trait objects that implement [`StateAtBlock`].
pub trait DynStateAtBlock {
    /// Retrieves the state at a given block.
    ///
    /// The state overrides are applied after the block they are associated
    /// with. The specified override of a nonce may be ignored to maintain
    /// validity.
    fn state_at_block_number(
        &self,
        block_number: u64,
        // Block number -> state overrides
        state_overrides: &BTreeMap<u64, StateOverride>,
    ) -> Result<Box<dyn DynState>, DynBlockchainError>;
}

impl<T: StateAtBlock<BlockchainError: 'static + std::error::Error>> DynStateAtBlock for T {
    fn state_at_block_number(
        &self,
        block_number: u64,
        state_overrides: &BTreeMap<u64, StateOverride>,
    ) -> Result<Box<dyn DynState>, DynBlockchainError> {
        self.state_at_block_number(block_number, state_overrides)
            .map_err(DynBlockchainError::new)
    }
}

/// Trait for dynamic trait objects that implement
/// [`TotalDifficultyByBlockHash`].
pub trait DynTotalDifficultyByBlockHash {
    /// Retrieves the total difficulty at the block with the provided hash.
    fn total_difficulty_by_hash(&self, hash: &B256) -> Result<Option<U256>, DynBlockchainError>;
}

impl<T: TotalDifficultyByBlockHash<Error: 'static + std::error::Error>>
    DynTotalDifficultyByBlockHash for T
{
    fn total_difficulty_by_hash(&self, hash: &B256) -> Result<Option<U256>, DynBlockchainError> {
        self.total_difficulty_by_hash(hash)
            .map_err(DynBlockchainError::new)
    }
}

/// Super-trait for dynamic trait objects that implement all blockchain
/// functionalities.
pub trait DynBlockchain<BlockReceiptT, BlockT: ?Sized, HardforkT, LocalBlockT, SignedTransactionT>:
    BlockchainMetadata<HardforkT>
    + DynBlockHashByNumber
    + DynBlockchainMetadataAtBlockNumber<HardforkT>
    + DynGetBlockchainBlock<BlockT, HardforkT>
    + DynGetBlockchainLogs
    + DynInsertBlock<BlockT, LocalBlockT, SignedTransactionT>
    + DynReceiptByTransactionHash<BlockReceiptT>
    + DynReserveBlocks
    + DynRevertToBlock
    + DynStateAtBlock
    + DynTotalDifficultyByBlockHash
{
}

impl<BlockReceiptT, BlockT: ?Sized, BlockchainT, HardforkT, LocalBlockT, SignedTransactionT>
    DynBlockchain<BlockReceiptT, BlockT, HardforkT, LocalBlockT, SignedTransactionT> for BlockchainT
where
    BlockchainT: BlockchainMetadata<HardforkT>
        + DynBlockHashByNumber
        + DynBlockchainMetadataAtBlockNumber<HardforkT>
        + DynGetBlockchainBlock<BlockT, HardforkT>
        + DynGetBlockchainLogs
        + DynInsertBlock<BlockT, LocalBlockT, SignedTransactionT>
        + DynReceiptByTransactionHash<BlockReceiptT>
        + DynReserveBlocks
        + DynRevertToBlock
        + DynStateAtBlock
        + DynTotalDifficultyByBlockHash,
{
}

impl<BlockReceiptT, BlockT: ?Sized, HardforkT, LocalBlockT, SignedTransactionT> BlockHashByNumber
    for dyn DynBlockchain<BlockReceiptT, BlockT, HardforkT, LocalBlockT, SignedTransactionT>
{
    type Error = DynBlockchainError;

    fn block_hash_by_number(&self, block_number: u64) -> Result<B256, Self::Error> {
        DynBlockHashByNumber::block_hash_by_number(self, block_number)
    }
}
