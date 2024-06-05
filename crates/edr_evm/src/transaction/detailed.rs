use std::sync::Arc;

use edr_eth::receipt::BlockReceipt;

use crate::{chain_spec::L1ChainSpec, ExecutableTransaction};

/// Wrapper struct for a transaction and its receipt.
pub struct DetailedTransaction<'t> {
    /// The transaction
    pub transaction: &'t ExecutableTransaction<L1ChainSpec>,
    /// The transaction's receipt
    pub receipt: &'t Arc<BlockReceipt>,
}
