use std::sync::Arc;

use edr_eth::{
    log::FilterLog,
    receipt::{BlockReceipt, Receipt},
};

/// Wrapper struct for a transaction and its receipt.
pub struct DetailedTransaction<'transaction, ExecutionReceiptT, SignedTransactionT>
where
    ExecutionReceiptT: Receipt<FilterLog>,
{
    /// The transaction
    pub transaction: &'transaction SignedTransactionT,
    /// The transaction's receipt
    pub receipt: &'transaction Arc<BlockReceipt<ExecutionReceiptT>>,
}
