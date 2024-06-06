use std::sync::Arc;

use edr_eth::{receipt::BlockReceipt, transaction};

/// Wrapper struct for a transaction and its receipt.
pub struct DetailedTransaction<'t> {
    /// The transaction
    pub transaction: &'t transaction::Signed,
    /// The transaction's receipt
    pub receipt: &'t Arc<BlockReceipt>,
}
