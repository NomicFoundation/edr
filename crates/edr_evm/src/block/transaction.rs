use std::sync::Arc;

use derive_where::derive_where;
use edr_eth::{chain_spec::L1ChainSpec, transaction::SignedTransaction as _, SpecId};
use edr_rpc_eth::RpcTypeFrom;

use super::SyncBlock;
use crate::{blockchain::BlockchainError, chain_spec::EvmSpec};

/// The result returned by requesting a transaction.
#[derive_where(Clone, Debug; ChainSpecT::Transaction)]
pub struct TransactionAndBlock<ChainSpecT: EvmSpec> {
    /// The transaction.
    pub transaction: ChainSpecT::Transaction,
    /// Block data in which the transaction is found if it has been mined.
    pub block_data: Option<BlockDataForTransaction<ChainSpecT>>,
    /// Whether the transaction is pending
    pub is_pending: bool,
}

/// Block metadata for a transaction.
#[derive_where(Clone, Debug)]
pub struct BlockDataForTransaction<ChainSpecT: EvmSpec> {
    /// The block in which the transaction is found.
    pub block: Arc<dyn SyncBlock<ChainSpecT, Error = BlockchainError<ChainSpecT>>>,
    /// The index of the transaction in the block.
    pub transaction_index: u64,
}

impl RpcTypeFrom<TransactionAndBlock<L1ChainSpec>> for edr_rpc_eth::TransactionWithSignature {
    type Hardfork = SpecId;

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
