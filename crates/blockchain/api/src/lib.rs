//! Types for Ethereum blockchains
#![warn(missing_docs)]

use std::{collections::BTreeMap, sync::Arc};

use auto_impl::auto_impl;
use edr_block_api::BlockAndTotalDifficulty;
use edr_primitives::{Address, HashSet, B256, U256};
use edr_receipt::log::FilterLog;
use edr_state_api::{StateDiff, StateOverride, SyncState};

/// Trait for retrieving a block's hash by number.
#[auto_impl(&, &mut, Box, Rc, Arc)]
pub trait BlockHash {
    /// The blockchain's error type.
    type Error;

    /// Retrieves the block hash at the provided number.
    fn block_hash_by_number(&self, block_number: u64) -> Result<B256, Self::Error>;
}

/// Trait for implementations of an Ethereum blockchain.
#[auto_impl(&)]
pub trait Blockchain<BlockT: ?Sized, BlockReceiptT, HardforkT> {
    /// The blockchain's error type
    type BlockchainError;

    /// The state's error type
    type StateError;

    /// Retrieves the block with the provided hash, if it exists.
    #[allow(clippy::type_complexity)]
    fn block_by_hash(&self, hash: &B256) -> Result<Option<Arc<BlockT>>, Self::BlockchainError>;

    /// Retrieves the block with the provided number, if it exists.
    #[allow(clippy::type_complexity)]
    fn block_by_number(&self, number: u64) -> Result<Option<Arc<BlockT>>, Self::BlockchainError>;

    /// Retrieves the block that contains a transaction with the provided hash,
    /// if it exists.
    #[allow(clippy::type_complexity)]
    fn block_by_transaction_hash(
        &self,
        transaction_hash: &B256,
    ) -> Result<Option<Arc<BlockT>>, Self::BlockchainError>;

    /// Retrieves the instances chain ID.
    fn chain_id(&self) -> u64;

    /// Retrieves the chain ID of the block at the provided number.
    /// The chain ID can be different in fork mode pre- and post-fork block
    /// number.
    fn chain_id_at_block_number(&self, _block_number: u64) -> Result<u64, Self::BlockchainError> {
        // Chain id only depends on the block number in fork mode
        Ok(self.chain_id())
    }

    /// Retrieves the last block in the blockchain.
    fn last_block(&self) -> Result<Arc<BlockT>, Self::BlockchainError>;

    /// Retrieves the last block number in the blockchain.
    fn last_block_number(&self) -> u64;

    /// Retrieves the logs that match the provided filter.
    fn logs(
        &self,
        from_block: u64,
        to_block: u64,
        addresses: &HashSet<Address>,
        normalized_topics: &[Option<Vec<B256>>],
    ) -> Result<Vec<FilterLog>, Self::BlockchainError>;

    /// Retrieves the network ID of the blockchain.
    fn network_id(&self) -> u64;

    /// Retrieves the receipt of the transaction with the provided hash, if it
    /// exists.
    fn receipt_by_transaction_hash(
        &self,
        transaction_hash: &B256,
    ) -> Result<Option<Arc<BlockReceiptT>>, Self::BlockchainError>;

    /// Retrieves the hardfork specification of the block at the provided
    /// number.
    fn spec_at_block_number(&self, block_number: u64) -> Result<HardforkT, Self::BlockchainError>;

    /// Retrieves the hardfork specification used for new blocks.
    fn hardfork(&self) -> HardforkT;

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

    /// Retrieves the total difficulty at the block with the provided hash.
    fn total_difficulty_by_hash(&self, hash: &B256) -> Result<Option<U256>, Self::BlockchainError>;
}

/// Trait for implementations of a mutable Ethereum blockchain
pub trait BlockchainMut<BlockT: ?Sized, LocalBlockT, SignedTransactionT> {
    /// The blockchain's error type
    type Error;

    /// Inserts the provided block into the blockchain, returning a reference to
    /// the inserted block.
    fn insert_block(
        &mut self,
        block: LocalBlockT,
        state_diff: StateDiff,
    ) -> Result<BlockAndTotalDifficulty<Arc<BlockT>, SignedTransactionT>, Self::Error>;

    /// Reserves the provided number of blocks, starting from the next block
    /// number.
    fn reserve_blocks(&mut self, additional: u64, interval: u64) -> Result<(), Self::Error>;

    /// Reverts to the block with the provided number, deleting all later
    /// blocks.
    fn revert_to_block(&mut self, block_number: u64) -> Result<(), Self::Error>;
}
