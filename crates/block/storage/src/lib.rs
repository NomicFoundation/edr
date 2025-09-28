//! Data structures for storing a blockchain's blocks in-memory.
#![warn(missing_docs)]

mod contiguous;
mod reservable;
mod sparse;

use edr_primitives::B256;

pub use self::{
    contiguous::ContiguousBlockStorage, reservable::ReservableSparseBlockStorage,
    sparse::SparseBlockStorage,
};

/// An error that occurs when trying to insert a block into storage.
#[derive(Debug, thiserror::Error)]
pub enum InsertError {
    /// Block already exists
    #[error("A block, with hash {block_hash} and number {block_number}, already exists.")]
    DuplicateBlock {
        /// The block's hash
        block_hash: B256,
        /// The block's number
        block_number: u64,
    },
    /// Receipt already exists
    #[error("A receipt with transaction hash {transaction_hash} already exists.")]
    DuplicateReceipt {
        /// Transaction hash of duplicated receipt
        transaction_hash: B256,
    },
    /// Transaction already exists
    #[error("A transaction with hash {hash} already exists.")]
    DuplicateTransaction {
        /// Hash of duplicated transaction
        hash: B256,
    },
}
