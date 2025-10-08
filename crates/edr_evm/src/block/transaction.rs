use std::sync::Arc;

use edr_chain_l1::{
    rpc::transaction::{L1RpcTransaction, L1RpcTransactionWithSignature},
    L1ChainSpec,
};
use edr_evm_spec::ChainSpec;
use edr_rpc_spec::RpcTypeFrom;
use edr_transaction::SignedTransaction as _;

/// Helper type for a chain-specific [`TransactionAndBlock`].
pub type TransactionAndBlockForChainSpec<ChainSpecT> = TransactionAndBlock<
    Arc<<ChainSpecT as RuntimeSpec>::Block>,
    <ChainSpecT as ChainSpec>::SignedTransaction,
>;

impl RpcTypeFrom<TransactionAndBlockForChainSpec<L1ChainSpec>> for L1RpcTransactionWithSignature {
    type Hardfork = edr_chain_l1::Hardfork;

    fn rpc_type_from(
        value: &TransactionAndBlockForChainSpec<L1ChainSpec>,
        hardfork: Self::Hardfork,
    ) -> Self {
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

        let transaction = L1RpcTransaction::new(
            &value.transaction,
            header,
            transaction_index,
            value.is_pending,
            hardfork,
        );
        let signature = value.transaction.signature();

        L1RpcTransactionWithSignature::new(
            transaction,
            signature.r(),
            signature.s(),
            signature.v(),
            signature.y_parity(),
        )
    }
}
