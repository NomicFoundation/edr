use core::fmt::Debug;
use std::{marker::PhantomData, sync::Arc};

use auto_impl::auto_impl;
use edr_block_header::{BlockHeader, PartialHeader, Withdrawal};
use edr_primitives::{B256, U256};
use edr_receipt::ReceiptTrait;

/// Trait for implementations of an Ethereum block.
#[auto_impl(Arc)]
pub trait Block<SignedTransactionT>: Debug {
    /// Returns the block's hash.
    fn block_hash(&self) -> &B256;

    /// Returns the block's header.
    fn header(&self) -> &BlockHeader;

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

/// Trait for locally mined blocks.
#[auto_impl(Arc)]
pub trait LocalBlock<BlockReceiptT> {
    /// Returns the receipts of the block's transactions.
    fn transaction_receipts(&self) -> &[BlockReceiptT];
}
