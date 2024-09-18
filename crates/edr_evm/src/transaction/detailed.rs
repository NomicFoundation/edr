use std::sync::Arc;

use edr_eth::{log::FilterLog, receipt::BlockReceipt};

use crate::chain_spec::RuntimeSpec;

/// Wrapper struct for a transaction and its receipt.
pub struct DetailedTransaction<'transaction, ChainSpecT: RuntimeSpec> {
    /// The transaction
    pub transaction: &'transaction ChainSpecT::Transaction,
    /// The transaction's receipt
    pub receipt: &'transaction Arc<BlockReceipt<ChainSpecT::ExecutionReceipt<FilterLog>>>,
}
