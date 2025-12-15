//! Types for Ethereum blockchains
#![warn(missing_docs)]

pub mod r#dyn;
pub mod sync;
pub mod utils;

use std::{collections::BTreeMap, sync::Arc};

use auto_impl::auto_impl;
use edr_block_api::BlockAndTotalDifficulty;
use edr_eip1559::BaseFeeParams;
use edr_eip7892::ScheduledBlobParams;
use edr_primitives::{Address, HashSet, B256, U256};
use edr_receipt::log::FilterLog;
use edr_state_api::{DynState, StateDiff, StateOverride};

/// Trait for retrieving a block's hash by number.
#[auto_impl(&, &mut, Box, Rc, Arc)]
pub trait BlockHashByNumber {
    /// The blockchain's error type.
    type Error;

    /// Retrieves the block hash at the provided number.
    fn block_hash_by_number(&self, block_number: u64) -> Result<B256, Self::Error>;
}

/// Trait for retrieving blockchain metadata.
#[auto_impl(&)]
pub trait BlockchainMetadata<HardforkT> {
    /// The blockchain's error type
    type Error;

    /// Retrieves the base fee parameters for the blockchain.
    fn base_fee_params(&self) -> &BaseFeeParams<HardforkT>;

    /// Retrieves the instances chain ID.
    fn chain_id(&self) -> u64;

    /// Retrieves the chain ID of the block at the provided number.
    /// The chain ID can be different in fork mode pre- and post-fork block
    /// number.
    fn chain_id_at_block_number(&self, _block_number: u64) -> Result<u64, Self::Error>;

    /// Retrieves the hardfork specification of the block at the provided
    /// number.
    fn spec_at_block_number(&self, block_number: u64) -> Result<HardforkT, Self::Error>;

    /// Retrieves the hardfork specification used for new blocks.
    fn hardfork(&self) -> HardforkT;

    /// Retrieves the last block number in the blockchain.
    fn last_block_number(&self) -> u64;

    /// Retrieves the minimum difficulty for the Ethash proof-of-work algorithm.
    fn min_ethash_difficulty(&self) -> u64;

    /// Retrieves the network ID of the blockchain.
    fn network_id(&self) -> u64;
}

/// Trait that defines Blob Parameter Only hardforks schedule for a blockchain
#[auto_impl(&)]
pub trait BlockchainScheduledBlobParams {
    /// Scheduled block parameter only hardforks ([EIP-7892])
    ///
    /// [EIP-7892]: https://eips.ethereum.org/EIPS/eip-7892
    fn scheduled_blob_params(&self) -> Option<&ScheduledBlobParams>;
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

/// Trait for retrieving logs from the blockchain.
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

/// Trait for retrieving a receipt by its transaction hash.
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

/// Trait for reserving blocks in the blockchain.
pub trait ReserveBlocks {
    /// The blockchain's error type
    type Error;

    /// Reserves the provided number of blocks, starting from the next block
    /// number.
    // TODO: https://github.com/NomicFoundation/edr/issues/1228
    // Analyze whether we can receive the BlockConfig here so blockachain does not
    // have to keep track of it
    fn reserve_blocks(&mut self, additional: u64, interval: u64) -> Result<(), Self::Error>;
}

/// Trait for reverting the blockchain to a previous block.
pub trait RevertToBlock {
    /// The blockchain's error type
    type Error;

    /// Reverts to the block with the provided number, deleting all later
    /// blocks.
    fn revert_to_block(&mut self, block_number: u64) -> Result<(), Self::Error>;
}

/// Trait for retrieving the state at a given block.
#[auto_impl(&)]
pub trait StateAtBlock {
    /// The blockchain's error type
    type BlockchainError;

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
    ) -> Result<Box<dyn DynState>, Self::BlockchainError>;
}

/// Trait for retrieving the total difficulty by its block hash.
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
>:
    BlockHashByNumberAndScheduledBlobParams<BlockchainErrorT>
    + BlockchainMetadata<HardforkT, Error = BlockchainErrorT>
    + GetBlockchainBlock<BlockT, HardforkT, Error = BlockchainErrorT>
    + GetBlockchainLogs<Error = BlockchainErrorT>
    + InsertBlock<BlockT, LocalBlockT, SignedTransactionT, Error = BlockchainErrorT>
    + ReceiptByTransactionHash<BlockReceiptT, Error = BlockchainErrorT>
    + ReserveBlocks<Error = BlockchainErrorT>
    + RevertToBlock<Error = BlockchainErrorT>
    + StateAtBlock<BlockchainError = BlockchainErrorT>
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
    >
    Blockchain<BlockReceiptT, BlockT, BlockchainErrorT, HardforkT, LocalBlockT, SignedTransactionT>
    for BlockchainT
where
    BlockchainT: BlockHashByNumberAndScheduledBlobParams<BlockchainErrorT>
        + BlockchainMetadata<HardforkT, Error = BlockchainErrorT>
        + GetBlockchainBlock<BlockT, HardforkT, Error = BlockchainErrorT>
        + GetBlockchainLogs<Error = BlockchainErrorT>
        + InsertBlock<BlockT, LocalBlockT, SignedTransactionT, Error = BlockchainErrorT>
        + ReceiptByTransactionHash<BlockReceiptT, Error = BlockchainErrorT>
        + ReserveBlocks<Error = BlockchainErrorT>
        + RevertToBlock<Error = BlockchainErrorT>
        + StateAtBlock<BlockchainError = BlockchainErrorT>
        + TotalDifficultyByBlockHash<Error = BlockchainErrorT>,
{
}

/// Supertrait for combining `BlockHashByNumber` together with
/// `BlockchainScheduledBlobParams`
pub trait BlockHashByNumberAndScheduledBlobParams<BlockchainErrorT>:
    BlockHashByNumber<Error = BlockchainErrorT> + BlockchainScheduledBlobParams
{
}

impl<BlockchainT, BlockchainErrorT> BlockHashByNumberAndScheduledBlobParams<BlockchainErrorT>
    for BlockchainT
where
    BlockchainT: BlockHashByNumber<Error = BlockchainErrorT> + BlockchainScheduledBlobParams,
{
}
