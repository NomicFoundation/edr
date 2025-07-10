use std::sync::Arc;

use edr_eth::spec::ChainSpec;

use crate::spec::RuntimeSpec;

/// Helper type for a chain-specific [`TransactionAndBlock`].
pub type TransactionAndBlockForChainSpec<ChainSpecT> = TransactionAndBlock<
    Arc<<ChainSpecT as RuntimeSpec>::Block>,
    <ChainSpecT as ChainSpec>::SignedTransaction,
>;

/// The result returned by requesting a transaction.
#[derive(Clone, Debug)]
pub struct TransactionAndBlock<BlockT, SignedTransactionT> {
    /// The transaction.
    pub transaction: SignedTransactionT,
    /// Block data in which the transaction is found if it has been mined.
    pub block_data: Option<BlockDataForTransaction<BlockT>>,
    /// Whether the transaction is pending
    pub is_pending: bool,
}

/// Block metadata for a transaction.
#[derive(Clone, Debug)]
pub struct BlockDataForTransaction<BlockT> {
    /// The block in which the transaction is found.
    pub block: BlockT,
    /// The index of the transaction in the block.
    pub transaction_index: u64,
}
