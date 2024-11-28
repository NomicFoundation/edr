use std::sync::Arc;

use edr_eth::{
    log::FilterLog,
    receipt::{BlockReceipt, Receipt},
};
use edr_utils::types::HigherKinded;

/// Wrapper struct for a transaction and its receipt.
pub struct DetailedTransaction<'transaction, ExecutionReceiptHigherKindedT, SignedTransactionT>
where
    ExecutionReceiptHigherKindedT: HigherKinded<FilterLog, Type: Receipt<FilterLog>>,
{
    /// The transaction
    pub transaction: &'transaction SignedTransactionT,
    /// The transaction's receipt
    pub receipt: &'transaction Arc<
        BlockReceipt<<ExecutionReceiptHigherKindedT as HigherKinded<FilterLog>>::Type>,
    >,
}
