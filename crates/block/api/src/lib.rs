use core::fmt::Debug;

use auto_impl::auto_impl;
use edr_block_header::{BlockHeader, Withdrawal};

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
use std::marker::PhantomData;

use edr_primitives::{B256, U256};

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
