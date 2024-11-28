use std::sync::Arc;

use alloy_rlp::RlpEncodable;
use derive_where::derive_where;
use edr_eth::{
    block::{self, Header, PartialHeader},
    eips::eip2718::TypedEnvelope,
    keccak256, l1,
    log::{ExecutionLog, FilterLog, FullBlockLog, ReceiptLog},
    receipt::{BlockReceipt, MapReceiptLogs, Receipt, TransactionReceipt},
    transaction::ExecutableTransaction,
    trie,
    withdrawal::Withdrawal,
    B256,
};
use edr_utils::types::HigherKinded;
use itertools::izip;

use crate::{
    blockchain::BlockchainError,
    spec::{RuntimeSpec, SyncRuntimeSpec},
    transaction::DetailedTransaction,
    Block, SyncBlock,
};

/// A locally mined block, which contains complete information.
#[derive(RlpEncodable)]
#[derive_where(Clone, Debug, PartialEq, Eq; <ExecutionReceiptHigherKindedT as HigherKinded<FilterLog>>::Type, SignedTransactionT)]
#[rlp(trailing)]
pub struct LocalBlock<
    ExecutionReceiptHigherKindedT: HigherKinded<ExecutionLog> + HigherKinded<FilterLog, Type: Receipt<FilterLog>>,
    SignedTransactionT: ExecutableTransaction + alloy_rlp::Encodable,
> {
    header: block::Header,
    transactions: Vec<SignedTransactionT>,
    #[rlp(skip)]
    transaction_receipts:
        Vec<Arc<BlockReceipt<<ExecutionReceiptHigherKindedT as HigherKinded<FilterLog>>::Type>>>,
    ommers: Vec<block::Header>,
    #[rlp(skip)]
    ommer_hashes: Vec<B256>,
    withdrawals: Option<Vec<Withdrawal>>,
    #[rlp(skip)]
    hash: B256,
}

impl<
        ExecutionReceiptHigherKindedT: HigherKinded<ExecutionLog> + HigherKinded<FilterLog, Type: Receipt<FilterLog>>,
        SignedTransactionT: ExecutableTransaction + alloy_rlp::Encodable,
    > LocalBlock<ExecutionReceiptHigherKindedT, SignedTransactionT>
{
    /// Constructs an empty block, i.e. no transactions.
    pub fn empty(spec_id: l1::SpecId, partial_header: PartialHeader) -> Self {
        let withdrawals = if spec_id >= l1::SpecId::SHANGHAI {
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
        transactions: Vec<SignedTransactionT>,
        transaction_receipts: Vec<
            TransactionReceipt<
                <ExecutionReceiptHigherKindedT as HigherKinded<ExecutionLog>>::Type,
                ExecutionLog,
            >,
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
        let transaction_receipts = transaction_to_block_receipts::<ExecutionReceiptHigherKindedT>(
            &hash,
            header.number,
            transaction_receipts,
        );

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
    ) -> &[Arc<BlockReceipt<<ExecutionReceiptHigherKindedT as HigherKinded<FilterLog>>::Type>>]
    {
        &self.transaction_receipts
    }

    /// Retrieves the block's transactions.
    pub fn detailed_transactions(
        &self,
    ) -> impl Iterator<Item = DetailedTransaction<'_, ExecutionReceiptHigherKindedT, SignedTransactionT>>
    {
        izip!(self.transactions.iter(), self.transaction_receipts.iter()).map(
            |(transaction, receipt)| DetailedTransaction {
                transaction,
                receipt,
            },
        )
    }
}

impl<
        BlockConversionErrorT,
        ExecutionReceiptHigherKindedT: HigherKinded<ExecutionLog> + HigherKinded<FilterLog, Type: Receipt<FilterLog>>,
        HardforkT,
        ReceiptConversionErrorT,
        SignedTransactionT: ExecutableTransaction,
    > Block<ExecutionReceiptHigherKindedT, SignedTransactionT>
    for LocalBlock<ExecutionReceiptHigherKindedT, SignedTransactionT>
where
    ExecutionReceiptHigherKindedT: HigherKinded<FilterLog, Type: Receipt<FilterLog>>,
{
    type Error = BlockchainError<BlockConversionErrorT, HardforkT, ReceiptConversionErrorT>;

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

    fn transactions(&self) -> &[SignedTransactionT] {
        &self.transactions
    }

    fn transaction_receipts(
        &self,
    ) -> Result<
        Vec<Arc<BlockReceipt<ChainSpecT::ExecutionReceipt<FilterLog>>>>,
        BlockchainError<BlockConversionErrorT, HardforkT, ReceiptConversionErrorT>,
    > {
        Ok(self.transaction_receipts.clone())
    }

    fn ommer_hashes(&self) -> &[B256] {
        self.ommer_hashes.as_slice()
    }

    fn withdrawals(&self) -> Option<&[Withdrawal]> {
        self.withdrawals.as_deref()
    }
}

fn transaction_to_block_receipts<ExecutionReceiptHigherKindedT>(
    block_hash: &B256,
    block_number: u64,
    receipts: Vec<
        TransactionReceipt<
            <ExecutionReceiptHigherKindedT as HigherKinded<ExecutionLog>>::Type,
            ExecutionLog,
        >,
    >,
) -> Vec<Arc<BlockReceipt<<ExecutionReceiptHigherKindedT as HigherKinded<FilterLog>>::Type>>>
where
    ExecutionReceiptHigherKindedT: HigherKinded<
            ExecutionLog,
            Type: MapReceiptLogs<
                ExecutionLog,
                FilterLog,
                <ExecutionReceiptHigherKindedT as HigherKinded<FilterLog>>::Type,
            >,
        > + HigherKinded<FilterLog, Type: Receipt<FilterLog>>,
{
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
    ChainSpecT: SyncRuntimeSpec,
{
    fn from(value: LocalBlock<ChainSpecT>) -> Self {
        Arc::new(value)
    }
}
