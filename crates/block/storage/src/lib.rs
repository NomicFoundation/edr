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
pub enum InsertBlockError {
    /// Block already exists
    #[error("A block, with hash {block_hash} and number {block_number}, already exists.")]
    DuplicateBlock {
        /// The block's hash
        block_hash: B256,
        /// The block's number
        block_number: u64,
    },
    /// An error that occurs when trying to insert a receipt into storage.
    #[error(transparent)]
    InsertReceiptError(#[from] InsertReceiptError),
    /// Transaction already exists
    #[error("A transaction with hash {hash} already exists.")]
    DuplicateTransaction {
        /// Hash of duplicated transaction
        hash: B256,
    },
}

/// An error that occurs when trying to insert a receipt into storage.
#[derive(Debug, thiserror::Error)]
pub enum InsertReceiptError {
    /// Receipt already exists
    #[error("A receipt with transaction hash {transaction_hash} already exists.")]
    Duplicate {
        /// Transaction hash of duplicated receipt
        transaction_hash: B256,
    },
}
