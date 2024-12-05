use std::sync::Arc;

use edr_eth::{
    log::FilterLog,
    receipt::{BlockReceipt, ExecutionReceipt},
};

/// Wrapper struct for a transaction and its receipt.
pub struct DetailedTransaction<'transaction, ExecutionReceiptT, SignedTransactionT>
where
    ExecutionReceiptT: ExecutionReceipt<FilterLog>,
{
    /// The transaction
    pub transaction: &'transaction SignedTransactionT,
    /// The transaction's receipt
    pub receipt: &'transaction Arc<BlockReceipt<ExecutionReceiptT>>,
}
