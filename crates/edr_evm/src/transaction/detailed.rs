use std::sync::Arc;

use edr_eth::{log::FilterLog, receipt::BlockReceipt};

use crate::chain_spec::EvmSpec;

/// Wrapper struct for a transaction and its receipt.
pub struct DetailedTransaction<'transaction, ChainSpecT: EvmSpec> {
    /// The transaction
    pub transaction: &'transaction ChainSpecT::Transaction,
    /// The transaction's receipt
    pub receipt: &'transaction Arc<BlockReceipt<ChainSpecT::ExecutionReceipt<FilterLog>>>,
}
