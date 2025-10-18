//! Types for Ethereum blockchains
#![warn(missing_docs)]

pub mod sync;
pub mod utils;

use std::{collections::BTreeMap, sync::Arc};

use auto_impl::auto_impl;
use edr_block_api::BlockAndTotalDifficulty;
use edr_primitives::{Address, HashSet, B256, U256};
use edr_receipt::log::FilterLog;
use edr_state_api::{StateDiff, StateOverride, SyncState};

/// Trait for retrieving a block's hash by number.
#[auto_impl(&, &mut, Box, Rc, Arc)]
pub trait BlockHashByNumber {
    /// The blockchain's error type.
    type Error;

    /// Retrieves the block hash at the provided number.
    fn block_hash_by_number(&self, block_number: u64) -> Result<B256, Self::Error>;
}

#[auto_impl(&)]
pub trait BlockchainMetadata<HardforkT> {
    /// The blockchain's error type
    type Error;

    /// Retrieves the instances chain ID.
    fn chain_id(&self) -> u64;

    /// Retrieves the chain ID of the block at the provided number.
    /// The chain ID can be different in fork mode pre- and post-fork block
    /// number.
    fn chain_id_at_block_number(&self, _block_number: u64) -> Result<u64, Self::Error> {
        // Chain id only depends on the block number in fork mode
        Ok(self.chain_id())
    }

    /// Retrieves the hardfork specification of the block at the provided
    /// number.
    fn spec_at_block_number(&self, block_number: u64) -> Result<HardforkT, Self::Error>;

    /// Retrieves the hardfork specification used for new blocks.
    fn hardfork(&self) -> HardforkT;

    /// Retrieves the last block number in the blockchain.
    fn last_block_number(&self) -> u64;

    /// Retrieves the network ID of the blockchain.
    fn network_id(&self) -> u64;
}

/// Trait for implementations of an Ethereum blockchain.
#[auto_impl(&)]
pub trait GetBlockchainBlock<BlockT: ?Sized, HardforkT> {
    /// The blockchain's error type
    type Error;

    /// Retrieves the block with the provided hash, if it exists.
    #[allow(clippy::type_complexity)]
    fn block_by_hash(&self, hash: &B256) -> Result<Option<Arc<BlockT>>, Self::Error>;

    /// Retrieves the block with the provided number, if it exists.
    #[allow(clippy::type_complexity)]
    fn block_by_number(&self, number: u64) -> Result<Option<Arc<BlockT>>, Self::Error>;

    /// Retrieves the block that contains a transaction with the provided hash,
    /// if it exists.
    #[allow(clippy::type_complexity)]
    fn block_by_transaction_hash(
        &self,
        transaction_hash: &B256,
    ) -> Result<Option<Arc<BlockT>>, Self::Error>;

    /// Retrieves the last block in the blockchain.
    fn last_block(&self) -> Result<Arc<BlockT>, Self::Error>;
}

#[auto_impl(&)]
pub trait GetBlockchainLogs {
    /// The blockchain's error type
    type Error;

    /// Retrieves the logs that match the provided filter.
    fn logs(
        &self,
        from_block: u64,
        to_block: u64,
        addresses: &HashSet<Address>,
        normalized_topics: &[Option<Vec<B256>>],
    ) -> Result<Vec<FilterLog>, Self::Error>;
}

/// Trait for implementations of a mutable Ethereum blockchain
pub trait InsertBlock<BlockT: ?Sized, LocalBlockT, SignedTransactionT> {
    /// The blockchain's error type
    type Error;

    /// Inserts the provided block into the blockchain, returning a reference to
    /// the inserted block.
    fn insert_block(
        &mut self,
        block: LocalBlockT,
        state_diff: StateDiff,
    ) -> Result<BlockAndTotalDifficulty<Arc<BlockT>, SignedTransactionT>, Self::Error>;
}

#[auto_impl(&)]
pub trait ReceiptByTransactionHash<BlockReceiptT> {
    /// The blockchain's error type
    type Error;

    /// Retrieves the receipt of the transaction with the provided hash, if it
    /// exists.
    fn receipt_by_transaction_hash(
        &self,
        transaction_hash: &B256,
    ) -> Result<Option<Arc<BlockReceiptT>>, Self::Error>;
}

pub trait ReserveBlocks {
    /// The blockchain's error type
    type Error;

    /// Reserves the provided number of blocks, starting from the next block
    /// number.
    fn reserve_blocks(&mut self, additional: u64, interval: u64) -> Result<(), Self::Error>;
}

pub trait RevertToBlock {
    /// The blockchain's error type
    type Error;

    /// Reverts to the block with the provided number, deleting all later
    /// blocks.
    fn revert_to_block(&mut self, block_number: u64) -> Result<(), Self::Error>;
}

#[auto_impl(&)]
pub trait StateAtBlock {
    /// The blockchain's error type
    type BlockchainError;

    /// The state's error type
    type StateError;

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
    ) -> Result<Box<dyn SyncState<Self::StateError>>, Self::BlockchainError>;
}

#[auto_impl(&)]
pub trait TotalDifficultyByBlockHash {
    /// The blockchain's error type
    type Error;

    /// Retrieves the total difficulty at the block with the provided hash.
    fn total_difficulty_by_hash(&self, hash: &B256) -> Result<Option<U256>, Self::Error>;
}

/// Super-trait for implementations of an Ethereum blockchain.
pub trait Blockchain<
    BlockReceiptT,
    BlockT: ?Sized,
    BlockchainErrorT,
    HardforkT,
    LocalBlockT,
    SignedTransactionT,
    StateErrorT,
>:
    BlockHashByNumber<Error = BlockchainErrorT>
    + BlockchainMetadata<HardforkT, Error = BlockchainErrorT>
    + GetBlockchainBlock<BlockT, HardforkT, Error = BlockchainErrorT>
    + GetBlockchainLogs<Error = BlockchainErrorT>
    + InsertBlock<BlockT, LocalBlockT, SignedTransactionT, Error = BlockchainErrorT>
    + ReceiptByTransactionHash<BlockReceiptT, Error = BlockchainErrorT>
    + ReserveBlocks<Error = BlockchainErrorT>
    + RevertToBlock<Error = BlockchainErrorT>
    + StateAtBlock<BlockchainError = BlockchainErrorT, StateError = StateErrorT>
    + TotalDifficultyByBlockHash<Error = BlockchainErrorT>
{
}

impl<
        BlockReceiptT,
        BlockT: ?Sized,
        BlockchainT,
        BlockchainErrorT,
        HardforkT,
        LocalBlockT,
        SignedTransactionT,
        StateErrorT,
    >
    Blockchain<
        BlockReceiptT,
        BlockT,
        BlockchainErrorT,
        HardforkT,
        LocalBlockT,
        SignedTransactionT,
        StateErrorT,
    > for BlockchainT
where
    BlockchainT: BlockHashByNumber<Error = BlockchainErrorT>
        + BlockchainMetadata<HardforkT, Error = BlockchainErrorT>
        + GetBlockchainBlock<BlockT, HardforkT, Error = BlockchainErrorT>
        + GetBlockchainLogs<Error = BlockchainErrorT>
        + InsertBlock<BlockT, LocalBlockT, SignedTransactionT, Error = BlockchainErrorT>
        + ReceiptByTransactionHash<BlockReceiptT, Error = BlockchainErrorT>
        + ReserveBlocks<Error = BlockchainErrorT>
        + RevertToBlock<Error = BlockchainErrorT>
        + StateAtBlock<BlockchainError = BlockchainErrorT, StateError = StateErrorT>
        + TotalDifficultyByBlockHash<Error = BlockchainErrorT>,
{
}
