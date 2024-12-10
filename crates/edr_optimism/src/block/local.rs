use std::sync::Arc;

use edr_eth::{block, log::FilterLog, withdrawal::Withdrawal, B256};
use edr_evm::{
    blockchain::BlockchainError, spec::ExecutionReceiptHigherKindedForChainSpec, Block,
    BlockReceipts, EthLocalBlock, RemoteBlockConversionError,
};
use revm_optimism::OptimismSpecId;

use crate::{eip2718::TypedEnvelope, receipt, rpc, transaction, L1BlockInfo, OptimismChainSpec};

#[derive(Debug)]
pub struct LocalBlock {
    pub(super) eth: EthLocalBlock<
        RemoteBlockConversionError<rpc::transaction::ConversionError>,
        receipt::Block,
        ExecutionReceiptHigherKindedForChainSpec<OptimismChainSpec>,
        OptimismSpecId,
        rpc::receipt::ConversionError,
        transaction::Signed,
    >,
    pub(super) l1_block_info: L1BlockInfo,
}

impl alloy_rlp::Encodable for LocalBlock {
    fn encode(&self, out: &mut dyn alloy_rlp::BufMut) {
        self.eth.encode(out);
    }

    fn length(&self) -> usize {
        self.eth.length()
    }
}

impl Block<transaction::Signed> for LocalBlock {
    fn block_hash(&self) -> &B256 {
        self.eth.block_hash()
    }

    fn header(&self) -> &block::Header {
        self.eth.header()
    }

    fn ommer_hashes(&self) -> &[B256] {
        self.eth.ommer_hashes()
    }

    fn rlp_size(&self) -> u64 {
        self.eth.rlp_size()
    }

    fn transactions(&self) -> &[transaction::Signed] {
        self.eth.transactions()
    }

    fn withdrawals(&self) -> Option<&[Withdrawal]> {
        self.eth.withdrawals()
    }
}

impl BlockReceipts<Arc<receipt::Block>> for LocalBlock {
    type Error = BlockchainError<
        RemoteBlockConversionError<rpc::transaction::ConversionError>,
        OptimismSpecId,
        rpc::receipt::ConversionError,
    >;

    fn fetch_transaction_receipts(&self) -> Result<Vec<Arc<receipt::Block>>, Self::Error> {
        self.eth.fetch_transaction_receipts()
    }
}

impl edr_evm::EmptyBlock<OptimismSpecId> for LocalBlock {
    fn empty(hardfork: OptimismSpecId, partial_header: block::PartialHeader) -> Self {
        Self {
            eth: EthLocalBlock::empty(hardfork, partial_header),
            l1_block_info: todo!(),
        }
    }
}

impl edr_evm::LocalBlock<Arc<receipt::Block>> for LocalBlock {
    fn transaction_receipts(&self) -> &[Arc<receipt::Block>] {
        self.eth.transaction_receipts()
    }
}
