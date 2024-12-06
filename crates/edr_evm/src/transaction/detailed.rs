use std::sync::Arc;

use edr_eth::{
    log::FilterLog,
    receipt::{BlockReceipt, ExecutionReceipt},
};

/// Wrapper struct for a transaction and its receipt.
pub struct DetailedTransaction<'transaction, SignedTransactionT, TransactionReceipT> {
    /// The transaction
    pub transaction: &'transaction SignedTransactionT,
    /// The transaction's receipt
    pub receipt: &'transaction TransactionReceipT,
}
