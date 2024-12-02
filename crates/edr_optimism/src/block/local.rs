use std::sync::Arc;

use edr_eth::{block, log::FilterLog, receipt::BlockReceipt, withdrawal::Withdrawal, B256};
use edr_evm::{
    blockchain::{BlockchainError, BlockchainErrorForChainSpec},
    spec::ExecutionReceiptHigherKindedForChainSpec,
    Block, BlockReceipts, EthLocalBlock, RemoteBlockConversionError, SyncBlock,
};
use revm_optimism::{L1BlockInfo, OptimismSpecId};

use crate::{eip2718::TypedEnvelope, receipt, rpc, transaction, OptimismChainSpec};

#[derive(Debug)]
pub struct LocalBlock {
    pub(super) eth: EthLocalBlock<
        RemoteBlockConversionError<rpc::transaction::ConversionError>,
        ExecutionReceiptHigherKindedForChainSpec<OptimismChainSpec>,
        OptimismSpecId,
        rpc::receipt::ConversionError,
        transaction::Signed,
    >,
    pub(super) l1_block_info: L1BlockInfo,
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

impl BlockReceipts<TypedEnvelope<receipt::Execution<FilterLog>>> for LocalBlock {
    type Error = BlockchainError<
        RemoteBlockConversionError<rpc::transaction::ConversionError>,
        OptimismSpecId,
        rpc::receipt::ConversionError,
    >;

    fn fetch_transaction_receipts(
        &self,
    ) -> Result<Vec<Arc<BlockReceipt<TypedEnvelope<receipt::Execution<FilterLog>>>>>, Self::Error>
    {
        self.eth.fetch_transaction_receipts()
    }
}

impl
    edr_evm::LocalBlock<
        TypedEnvelope<receipt::Execution<FilterLog>>,
        OptimismSpecId,
        transaction::Signed,
    > for LocalBlock
{
    fn empty(hardfork: OptimismSpecId, partial_header: block::PartialHeader) -> Self {
        Self {
            eth: EthLocalBlock::empty(hardfork, partial_header),
            l1_block_info: todo!(),
        }
    }

    fn transaction_receipts(
        &self,
    ) -> &[Arc<BlockReceipt<TypedEnvelope<receipt::Execution<FilterLog>>>>] {
        self.eth.transaction_receipts()
    }
}

impl From<LocalBlock>
    for Arc<
        dyn SyncBlock<
            TypedEnvelope<receipt::Execution<FilterLog>>,
            transaction::Signed,
            Error = BlockchainErrorForChainSpec<OptimismChainSpec>,
        >,
    >
{
    fn from(block: LocalBlock) -> Self {
        Arc::new(block)
    }
}
