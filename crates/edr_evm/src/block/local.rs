use std::sync::Arc;

use alloy_rlp::RlpEncodable;
use derive_where::derive_where;
use edr_eth::{
    block::{self, Header, PartialHeader},
    log::{ExecutionLog, FilterLog, FullBlockLog, ReceiptLog},
    receipt::{BlockReceipt, MapReceiptLogs as _, TransactionReceipt},
    transaction::ExecutableTransaction,
    trie,
    withdrawal::Withdrawal,
    SpecId, B256,
};
use itertools::izip;
use revm::primitives::keccak256;

use crate::{
    blockchain::BlockchainError,
    chain_spec::{ChainSpec, SyncChainSpec},
    transaction::DetailedTransaction,
    Block, SyncBlock,
};

/// A locally mined block, which contains complete information.
#[derive(PartialEq, Eq, RlpEncodable)]
#[derive_where(Clone, Debug; ChainSpecT::ExecutionReceipt<FilterLog>, ChainSpecT::Transaction)]
#[rlp(trailing)]
pub struct LocalBlock<ChainSpecT: ChainSpec> {
    header: block::Header,
    transactions: Vec<ChainSpecT::Transaction>,
    #[rlp(skip)]
    transaction_receipts: Vec<Arc<BlockReceipt<ChainSpecT::ExecutionReceipt<FilterLog>>>>,
    ommers: Vec<block::Header>,
    #[rlp(skip)]
    ommer_hashes: Vec<B256>,
    withdrawals: Option<Vec<Withdrawal>>,
    #[rlp(skip)]
    hash: B256,
}

impl<ChainSpecT: ChainSpec> LocalBlock<ChainSpecT> {
    /// Constructs an empty block, i.e. no transactions.
    pub fn empty(spec_id: ChainSpecT::Hardfork, partial_header: PartialHeader) -> Self {
        let withdrawals = if spec_id.into() >= SpecId::SHANGHAI {
            Some(Vec::default())
        } else {
            None
        };

        Self::new(
            partial_header,
            Vec::new(),
            Vec::new(),
            Vec::new(),
            withdrawals,
        )
    }

    /// Constructs a new instance with the provided data.
    pub fn new(
        partial_header: PartialHeader,
        transactions: Vec<ChainSpecT::Transaction>,
        transaction_receipts: Vec<
            TransactionReceipt<ChainSpecT::ExecutionReceipt<ExecutionLog>, ExecutionLog>,
        >,
        ommers: Vec<Header>,
        withdrawals: Option<Vec<Withdrawal>>,
    ) -> Self {
        let ommer_hashes = ommers.iter().map(Header::hash).collect::<Vec<_>>();
        let ommers_hash = keccak256(alloy_rlp::encode(&ommers));
        let transactions_root =
            trie::ordered_trie_root(transactions.iter().map(ExecutableTransaction::rlp_encoding));

        let withdrawals_root = withdrawals
            .as_ref()
            .map(|w| trie::ordered_trie_root(w.iter().map(alloy_rlp::encode)));

        let header = Header::new(
            partial_header,
            ommers_hash,
            transactions_root,
            withdrawals_root,
        );

        let hash = header.hash();
        let transaction_receipts =
            transaction_to_block_receipts::<ChainSpecT>(&hash, header.number, transaction_receipts);

        Self {
            header,
            transactions,
            transaction_receipts,
            ommers,
            ommer_hashes,
            withdrawals,
            hash,
        }
    }

    /// Returns the receipts of the block's transactions.
    pub fn transaction_receipts(
        &self,
    ) -> &[Arc<BlockReceipt<ChainSpecT::ExecutionReceipt<FilterLog>>>] {
        &self.transaction_receipts
    }

    /// Retrieves the block's transactions.
    pub fn detailed_transactions(
        &self,
    ) -> impl Iterator<Item = DetailedTransaction<'_, ChainSpecT>> {
        izip!(self.transactions.iter(), self.transaction_receipts.iter()).map(
            |(transaction, receipt)| DetailedTransaction {
                transaction,
                receipt,
            },
        )
    }
}

impl<ChainSpecT: ChainSpec> Block<ChainSpecT> for LocalBlock<ChainSpecT> {
    type Error = BlockchainError<ChainSpecT>;

    fn hash(&self) -> &B256 {
        &self.hash
    }

    fn header(&self) -> &block::Header {
        &self.header
    }

    fn rlp_size(&self) -> u64 {
        alloy_rlp::encode(self)
            .len()
            .try_into()
            .expect("usize fits into u64")
    }

    fn transactions(&self) -> &[ChainSpecT::Transaction] {
        &self.transactions
    }

    fn transaction_receipts(
        &self,
    ) -> Result<Vec<Arc<BlockReceipt<ChainSpecT::ExecutionReceipt<FilterLog>>>>, Self::Error> {
        Ok(self.transaction_receipts.clone())
    }

    fn ommer_hashes(&self) -> &[B256] {
        self.ommer_hashes.as_slice()
    }

    fn withdrawals(&self) -> Option<&[Withdrawal]> {
        self.withdrawals.as_deref()
    }
}

fn transaction_to_block_receipts<ChainSpecT: ChainSpec>(
    block_hash: &B256,
    block_number: u64,
    receipts: Vec<TransactionReceipt<ChainSpecT::ExecutionReceipt<ExecutionLog>, ExecutionLog>>,
) -> Vec<Arc<BlockReceipt<ChainSpecT::ExecutionReceipt<FilterLog>>>> {
    let mut log_index = 0;

    receipts
        .into_iter()
        .enumerate()
        .map(|(transaction_index, receipt)| {
            let transaction_index = transaction_index as u64;
            let transaction_hash = receipt.transaction_hash;

            Arc::new(BlockReceipt {
                inner: receipt.map_logs(|log| {
                    FilterLog {
                        inner: FullBlockLog {
                            inner: ReceiptLog {
                                inner: log,
                                transaction_hash,
                            },
                            block_hash: *block_hash,
                            block_number,
                            log_index: {
                                let index = log_index;
                                log_index += 1;
                                index
                            },
                            transaction_index,
                        },
                        // Assuming a local block is never reorged out.
                        removed: false,
                    }
                }),
                block_hash: *block_hash,
                block_number,
            })
        })
        .collect()
}

impl<ChainSpecT> From<LocalBlock<ChainSpecT>>
    for Arc<dyn SyncBlock<ChainSpecT, Error = BlockchainError<ChainSpecT>>>
where
    ChainSpecT: SyncChainSpec,
{
    fn from(value: LocalBlock<ChainSpecT>) -> Self {
        Arc::new(value)
    }
}
