use core::fmt::Debug;
use std::{marker::PhantomData, sync::Arc};

use alloy_rlp::Encodable as _;
use derive_where::derive_where;
use edr_eth::{
    block::{self, Header, PartialHeader},
    keccak256, l1,
    log::{ExecutionLog, FilterLog, FullBlockLog, ReceiptLog},
    receipt::{BlockReceipt, MapReceiptLogs, TransactionReceipt},
    spec::{ChainSpec, HardforkTrait},
    transaction::ExecutableTransaction,
    trie,
    withdrawal::Withdrawal,
    B256,
};
use edr_utils::types::HigherKinded;
use itertools::izip;

use crate::{
    block::{BlockReceipts, LocalBlock},
    blockchain::BlockchainError,
    spec::{
        ExecutionReceiptHigherKindedBounds, ExecutionReceiptHigherKindedForChainSpec, RuntimeSpec,
    },
    transaction::DetailedTransaction,
    Block, SyncBlock,
};

/// Helper type for a local Ethereum block for a given chain spec.
pub type EthLocalBlockForChainSpec<ChainSpecT> = EthLocalBlock<
    <ChainSpecT as RuntimeSpec>::RpcBlockConversionError,
    ExecutionReceiptHigherKindedForChainSpec<ChainSpecT>,
    <ChainSpecT as ChainSpec>::Hardfork,
    <ChainSpecT as RuntimeSpec>::RpcReceiptConversionError,
    <ChainSpecT as ChainSpec>::SignedTransaction,
>;

/// A locally mined block, which contains complete information.
#[derive_where(Clone, Debug, PartialEq, Eq; <ExecutionReceiptHigherKindedT as HigherKinded<FilterLog>>::Type, SignedTransactionT)]
pub struct EthLocalBlock<
    BlockConversionErrorT,
    ExecutionReceiptHigherKindedT: ExecutionReceiptHigherKindedBounds,
    HardforkT: HardforkTrait,
    ReceiptConversionErrorT,
    SignedTransactionT,
> {
    header: block::Header,
    transactions: Vec<SignedTransactionT>,
    transaction_receipts:
        Vec<Arc<BlockReceipt<<ExecutionReceiptHigherKindedT as HigherKinded<FilterLog>>::Type>>>,
    ommers: Vec<block::Header>,
    ommer_hashes: Vec<B256>,
    withdrawals: Option<Vec<Withdrawal>>,
    hash: B256,
    phantom: PhantomData<(BlockConversionErrorT, HardforkT, ReceiptConversionErrorT)>,
}

impl<
        BlockConversionErrorT,
        ExecutionReceiptHigherKindedT: ExecutionReceiptHigherKindedBounds,
        HardforkT: HardforkTrait,
        ReceiptConversionErrorT,
        SignedTransactionT: Debug + ExecutableTransaction,
    >
    EthLocalBlock<
        BlockConversionErrorT,
        ExecutionReceiptHigherKindedT,
        HardforkT,
        ReceiptConversionErrorT,
        SignedTransactionT,
    >
{
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
            phantom: PhantomData,
        }
    }

    /// Retrieves the block's transactions.
    pub fn detailed_transactions(
        &self,
    ) -> impl Iterator<
        Item = DetailedTransaction<
            '_,
            <ExecutionReceiptHigherKindedT as HigherKinded<FilterLog>>::Type,
            SignedTransactionT,
        >,
    > {
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
        ExecutionReceiptHigherKindedT: ExecutionReceiptHigherKindedBounds,
        HardforkT: HardforkTrait,
        ReceiptConversionErrorT,
        SignedTransactionT: Debug + alloy_rlp::Encodable,
    >
    EthLocalBlock<
        BlockConversionErrorT,
        ExecutionReceiptHigherKindedT,
        HardforkT,
        ReceiptConversionErrorT,
        SignedTransactionT,
    >
{
    fn rlp_payload_length(&self) -> usize {
        self.header.length()
            + self.transactions.length()
            + self.ommers.length()
            + self
                .withdrawals
                .as_ref()
                .map_or(0, alloy_rlp::Encodable::length)
    }
}

impl<
        BlockConversionErrorT,
        ExecutionReceiptHigherKindedT: ExecutionReceiptHigherKindedBounds,
        HardforkT: HardforkTrait,
        ReceiptConversionErrorT,
        SignedTransactionT: Debug + alloy_rlp::Encodable,
    > Block<SignedTransactionT>
    for EthLocalBlock<
        BlockConversionErrorT,
        ExecutionReceiptHigherKindedT,
        HardforkT,
        ReceiptConversionErrorT,
        SignedTransactionT,
    >
{
    fn block_hash(&self) -> &B256 {
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

    fn ommer_hashes(&self) -> &[B256] {
        self.ommer_hashes.as_slice()
    }

    fn withdrawals(&self) -> Option<&[Withdrawal]> {
        self.withdrawals.as_deref()
    }
}

impl<
        BlockConversionErrorT,
        ExecutionReceiptHigherKindedT: ExecutionReceiptHigherKindedBounds,
        HardforkT: HardforkTrait,
        ReceiptConversionErrorT,
        SignedTransactionT: Debug + alloy_rlp::Encodable,
    > BlockReceipts<<ExecutionReceiptHigherKindedT as HigherKinded<FilterLog>>::Type>
    for EthLocalBlock<
        BlockConversionErrorT,
        ExecutionReceiptHigherKindedT,
        HardforkT,
        ReceiptConversionErrorT,
        SignedTransactionT,
    >
{
    type Error = BlockchainError<BlockConversionErrorT, HardforkT, ReceiptConversionErrorT>;

    fn fetch_transaction_receipts(
        &self,
    ) -> Result<
        Vec<Arc<BlockReceipt<<ExecutionReceiptHigherKindedT as HigherKinded<FilterLog>>::Type>>>,
        BlockchainError<BlockConversionErrorT, HardforkT, ReceiptConversionErrorT>,
    > {
        Ok(self.transaction_receipts.clone())
    }
}

impl<
        BlockConversionErrorT,
        ExecutionReceiptHigherKindedT: ExecutionReceiptHigherKindedBounds,
        HardforkT: HardforkTrait,
        ReceiptConversionErrorT,
        SignedTransactionT: Debug + ExecutableTransaction + alloy_rlp::Encodable,
    >
    LocalBlock<
        <ExecutionReceiptHigherKindedT as HigherKinded<FilterLog>>::Type,
        HardforkT,
        SignedTransactionT,
    >
    for EthLocalBlock<
        BlockConversionErrorT,
        ExecutionReceiptHigherKindedT,
        HardforkT,
        ReceiptConversionErrorT,
        SignedTransactionT,
    >
{
    fn empty(spec_id: HardforkT, partial_header: PartialHeader) -> Self {
        let withdrawals = if spec_id.into() >= l1::SpecId::SHANGHAI {
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

    fn transaction_receipts(
        &self,
    ) -> &[Arc<BlockReceipt<<ExecutionReceiptHigherKindedT as HigherKinded<FilterLog>>::Type>>]
    {
        &self.transaction_receipts
    }
}

impl<
        BlockConversionErrorT,
        ExecutionReceiptHigherKindedT: ExecutionReceiptHigherKindedBounds,
        HardforkT: HardforkTrait,
        ReceiptConversionErrorT,
        SignedTransactionT: Debug + alloy_rlp::Encodable,
    > alloy_rlp::Encodable
    for EthLocalBlock<
        BlockConversionErrorT,
        ExecutionReceiptHigherKindedT,
        HardforkT,
        ReceiptConversionErrorT,
        SignedTransactionT,
    >
{
    fn encode(&self, out: &mut dyn alloy_rlp::BufMut) {
        alloy_rlp::Header {
            list: true,
            payload_length: self.rlp_payload_length(),
        }
        .encode(out);

        self.header.encode(out);
        self.transactions.encode(out);
        self.ommers.encode(out);

        if let Some(withdrawals) = &self.withdrawals {
            withdrawals.encode(out);
        }
    }

    fn length(&self) -> usize {
        let payload_length = self.rlp_payload_length();
        payload_length + alloy_rlp::length_of_length(payload_length)
    }
}

fn transaction_to_block_receipts<
    ExecutionReceiptHigherKindedT: ExecutionReceiptHigherKindedBounds,
>(
    block_hash: &B256,
    block_number: u64,
    receipts: Vec<
        TransactionReceipt<
            <ExecutionReceiptHigherKindedT as HigherKinded<ExecutionLog>>::Type,
            ExecutionLog,
        >,
    >,
) -> Vec<Arc<BlockReceipt<<ExecutionReceiptHigherKindedT as HigherKinded<FilterLog>>::Type>>> {
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

impl<
        BlockConversionErrorT: Send + Sync + 'static,
        ExecutionReceiptHigherKindedT: ExecutionReceiptHigherKindedBounds + HigherKinded<FilterLog, Type: Send + Sync> + 'static,
        HardforkT: HardforkTrait + Send + Sync + 'static,
        ReceiptConversionErrorT: Send + Sync + 'static,
        SignedTransactionT: Debug + alloy_rlp::Encodable + Send + Sync + 'static,
    >
    From<
        EthLocalBlock<
            BlockConversionErrorT,
            ExecutionReceiptHigherKindedT,
            HardforkT,
            ReceiptConversionErrorT,
            SignedTransactionT,
        >,
    >
    for Arc<
        dyn SyncBlock<
            <ExecutionReceiptHigherKindedT as HigherKinded<FilterLog>>::Type,
            SignedTransactionT,
            Error = BlockchainError<BlockConversionErrorT, HardforkT, ReceiptConversionErrorT>,
        >,
    >
{
    fn from(
        value: EthLocalBlock<
            BlockConversionErrorT,
            ExecutionReceiptHigherKindedT,
            HardforkT,
            ReceiptConversionErrorT,
            SignedTransactionT,
        >,
    ) -> Self {
        Arc::new(value)
    }
}
