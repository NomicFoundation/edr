use std::sync::Arc;

use edr_eth::receipt::BlockReceipt;

use crate::chain_spec::ChainSpec;

/// Wrapper struct for a transaction and its receipt.
pub struct DetailedTransaction<'transaction, ChainSpecT: ChainSpec> {
    /// The transaction
    pub transaction: &'transaction ChainSpecT::SignedTransaction,
    /// The transaction's receipt
    pub receipt: &'transaction Arc<BlockReceipt>,
}
