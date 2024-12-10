use edr_eth::{
    log::FilterLog,
    receipt::{ReceiptFactory, TransactionReceipt},
    B256,
};
use edr_evm::EthBlockReceiptFactory;
use op_alloy_rpc_types::receipt::L1BlockInfo;

use crate::{eip2718::TypedEnvelope, receipt};

/// Block receipt factory for Optimism.
pub struct BlockReceiptFactory {
    pub(crate) l1_block_info: L1BlockInfo,
}

impl ReceiptFactory<TypedEnvelope<receipt::Execution<FilterLog>>> for BlockReceiptFactory {
    type Output = receipt::Block;

    fn create_receipt(
        &self,
        transaction_receipt: TransactionReceipt<TypedEnvelope<receipt::Execution<FilterLog>>>,
        block_hash: &B256,
        block_number: u64,
    ) -> Self::Output {
        let l1_block_info = if matches!(
            transaction_receipt.inner,
            TypedEnvelope::Legacy(_) | TypedEnvelope::Deposit(_)
        ) {
            None
        } else {
            Some(self.l1_block_info)
        };

        let eth = {
            let receipt_factory = EthBlockReceiptFactory::default();
            receipt_factory.create_receipt(transaction_receipt, block_hash, block_number)
        };

        receipt::Block { eth, l1_block_info }
    }
}
