//! Ethereum block API
#![warn(missing_docs)]

mod genesis;

use core::fmt::Debug;
use std::{marker::PhantomData, sync::Arc};

use auto_impl::auto_impl;
use edr_block_header::{BlockHeader, PartialHeader, Withdrawal};
use edr_chain_spec::EvmSpecId;
use edr_primitives::{B256, U256};
use edr_receipt::ReceiptTrait;

pub use self::genesis::{GenesisBlockFactory, GenesisBlockOptions, SyncGenesisBlockFactory};

/// Trait for implementations of an Ethereum block.
#[auto_impl(Arc)]
pub trait Block<SignedTransactionT>: Debug {
    /// Returns the block's hash.
    fn block_hash(&self) -> &B256;

    /// Returns the block's header.
    fn block_header(&self) -> &BlockHeader;

    /// Ommer/uncle block hashes.
    fn ommer_hashes(&self) -> &[B256];

    /// The length of the RLP encoding of this block in bytes.
    fn rlp_size(&self) -> u64;

    /// Returns the block's transactions.
    fn transactions(&self) -> &[SignedTransactionT];

    /// Withdrawals
    fn withdrawals(&self) -> Option<&[Withdrawal]>;
}

/// The result returned by requesting a block by number.
#[derive(Clone, Debug)]
pub struct BlockAndTotalDifficulty<BlockT, SignedTransactionT> {
    /// The block
    pub block: BlockT,
    /// The total difficulty with the block
    pub total_difficulty: Option<U256>,
    phantom: PhantomData<SignedTransactionT>,
}

impl<BlockT, SignedTransactionT> BlockAndTotalDifficulty<BlockT, SignedTransactionT> {
    /// Creates a new block and total difficulty.
    pub fn new(block: BlockT, total_difficulty: Option<U256>) -> Self {
        Self {
            block,
            total_difficulty,
            phantom: PhantomData,
        }
    }
}

/// Trait for fetching the receipts of a block's transactions.
#[auto_impl(Arc)]
pub trait BlockReceipts<BlockReceiptT: ReceiptTrait> {
    /// The blockchain error type.
    type Error;

    /// Fetches the receipts of the block's transactions.
    ///
    /// This may block if the receipts are stored remotely.
    fn fetch_transaction_receipts(&self) -> Result<Vec<BlockReceiptT>, Self::Error>;
}

/// Trait for creating an empty block.
pub trait EmptyBlock<HardforkT> {
    /// Constructs an empty block.
    fn empty(hardfork: HardforkT, partial_header: PartialHeader) -> Self;
}

impl<BlockT: EmptyBlock<HardforkT>, HardforkT> EmptyBlock<HardforkT> for Arc<BlockT> {
    fn empty(hardfork: HardforkT, partial_header: PartialHeader) -> Self {
        Arc::new(BlockT::empty(hardfork, partial_header))
    }
}

/// A type containing the relevant data for an Ethereum block.
pub struct EthBlockData<SignedTransactionT> {
    /// The block's header.
    pub header: BlockHeader,
    /// The block's transactions.
    pub transactions: Vec<SignedTransactionT>,
    /// The hashes of the block's ommers.
    pub ommer_hashes: Vec<B256>,
    /// The staking withdrawals.
    pub withdrawals: Option<Vec<Withdrawal>>,
    /// The block's hash.
    pub hash: B256,
    /// The length of the RLP encoding of this block in bytes.
    pub rlp_size: u64,
}

/// Trait for locally mined blocks.
#[auto_impl(Arc)]
pub trait LocalBlock<BlockReceiptT> {
    /// Returns the receipts of the block's transactions.
    fn transaction_receipts(&self) -> &[BlockReceiptT];
}

/// Error due to an invalid next block.
#[derive(Debug, thiserror::Error)]
pub enum BlockValidityError {
    /// Invalid block number
    #[error("Invalid block number: {actual}. Expected: {expected}.")]
    InvalidBlockNumber {
        /// Provided block number
        actual: u64,
        /// Expected block number
        expected: u64,
    },
    /// Invalid parent hash
    #[error("Invalid parent hash: {actual}. Expected: {expected}.")]
    InvalidParentHash {
        /// Provided parent hash
        actual: B256,
        /// Expected parent hash
        expected: B256,
    },
    /// Missing withdrawals for post-Shanghai blockchain
    #[error("Missing withdrawals for post-Shanghai blockchain")]
    MissingWithdrawals,
}

/// Validates whether a block is a valid next block.
pub fn validate_next_block<HardforkT: Into<EvmSpecId>, SignedTransactionT>(
    spec_id: HardforkT,
    last_block: &dyn Block<SignedTransactionT>,
    next_block: &dyn Block<SignedTransactionT>,
) -> Result<(), BlockValidityError> {
    let last_header = last_block.block_header();
    let next_header = next_block.block_header();

    let next_block_number = last_header.number + 1;
    if next_header.number != next_block_number {
        return Err(BlockValidityError::InvalidBlockNumber {
            actual: next_header.number,
            expected: next_block_number,
        });
    }

    if next_header.parent_hash != *last_block.block_hash() {
        return Err(BlockValidityError::InvalidParentHash {
            actual: next_header.parent_hash,
            expected: *last_block.block_hash(),
        });
    }

    if spec_id.into() >= EvmSpecId::SHANGHAI && next_header.withdrawals_root.is_none() {
        return Err(BlockValidityError::MissingWithdrawals);
    }

    Ok(())
}
