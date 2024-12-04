use std::sync::Arc;

use derive_where::derive_where;
use edr_eth::{
    l1::{self, L1ChainSpec},
    log::FilterLog,
    receipt::{BlockReceipt, Receipt},
    transaction::SignedTransaction as _,
};
use edr_rpc_eth::{RpcSpec, RpcTypeFrom};

use super::SyncBlock;
use crate::{blockchain::BlockchainErrorForChainSpec, spec::RuntimeSpec};

/// Helper type for a chain-specific [`BlockAndTransactionReceipt`].
pub type TransactionReceiptAndBlockForChainSpec<ChainSpecT> = TransactionReceiptAndBlock<
    <ChainSpecT as RuntimeSpec>::Block,
    <ChainSpecT as RpcSpec>::ExecutionReceipt<FilterLog>,
>;

/// A transaction receipt and the block in which it is found.
#[derive(Debug)]
#[derive_where(Clone)]
pub struct TransactionReceiptAndBlock<BlockT: ?Sized, ExecutionReceiptT: Receipt<FilterLog>> {
    /// The block in which the transaction is found.
    pub block: Arc<BlockT>,
    /// The receipt.
    pub receipt: Arc<BlockReceipt<ExecutionReceiptT>>,
}

/// The result returned by requesting a transaction.
#[derive_where(Clone, Debug; ChainSpecT::SignedTransaction)]
pub struct TransactionAndBlock<ChainSpecT: RuntimeSpec> {
    /// The transaction.
    pub transaction: ChainSpecT::SignedTransaction,
    /// Block data in which the transaction is found if it has been mined.
    pub block_data: Option<BlockDataForTransaction<ChainSpecT>>,
    /// Whether the transaction is pending
    pub is_pending: bool,
}

impl RpcTypeFrom<TransactionReceiptAndBlockForChainSpec<L1ChainSpec>>
    for edr_rpc_eth::receipt::Block
{
    type Hardfork = l1::SpecId;

    fn rpc_type_from(
        value: &TransactionReceiptAndBlockForChainSpec<L1ChainSpec>,
        hardfork: Self::Hardfork,
    ) -> Self {
        Self::rpc_type_from(value.receipt.as_ref(), hardfork)
    }
}

/// Block metadata for a transaction.
#[derive_where(Clone, Debug)]
pub struct BlockDataForTransaction<ChainSpecT: RuntimeSpec> {
    /// The block in which the transaction is found.
    pub block: Arc<ChainSpecT::Block>,
    /// The index of the transaction in the block.
    pub transaction_index: u64,
}

impl RpcTypeFrom<TransactionAndBlock<L1ChainSpec>> for edr_rpc_eth::TransactionWithSignature {
    type Hardfork = l1::SpecId;

    fn rpc_type_from(value: &TransactionAndBlock<L1ChainSpec>, hardfork: Self::Hardfork) -> Self {
        let (header, transaction_index) = value
            .block_data
            .as_ref()
            .map(
                |BlockDataForTransaction {
                     block,
                     transaction_index,
                 }| (block.header(), *transaction_index),
            )
            .unzip();

        let transaction = edr_rpc_eth::Transaction::new(
            &value.transaction,
            header,
            transaction_index,
            value.is_pending,
            hardfork,
        );
        let signature = value.transaction.signature();

        edr_rpc_eth::TransactionWithSignature::new(
            transaction,
            signature.r(),
            signature.s(),
            signature.v(),
            signature.y_parity(),
        )
    }
}
