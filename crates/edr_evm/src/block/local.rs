use core::fmt::Debug;
use std::{
    marker::PhantomData,
    sync::Arc,
    time::{SystemTime, UNIX_EPOCH},
};

use alloy_rlp::Encodable as _;
use derive_where::derive_where;
use edr_eip1559::BaseFeeParams;
use edr_eth::{
    block::{self, Header, HeaderOverrides, PartialHeader},
    trie,
    withdrawal::Withdrawal,
    B256, KECCAK_EMPTY,
};
use edr_evm_spec::{
    ChainHardfork, ChainSpec, EthHeaderConstants, EvmSpecId, ExecutableTransaction,
};
use edr_receipt::{
    log::{ExecutionLog, FilterLog, FullBlockLog, ReceiptLog},
    MapReceiptLogs, ReceiptFactory, ReceiptTrait, TransactionReceipt,
};
use edr_utils::types::TypeConstructor;
use itertools::izip;

use crate::{
    block::{BlockReceipts, EmptyBlock, LocalBlock},
    blockchain::BlockchainError,
    spec::{
        ExecutionReceiptTypeConstructorBounds, ExecutionReceiptTypeConstructorForChainSpec,
        RuntimeSpec,
    },
    state::{StateCommit as _, StateDebug as _, StateDiff, TrieState},
    transaction::DetailedTransaction,
    Block, GenesisBlockOptions,
};

/// An error that occurs upon creation of an [`EthLocalBlock`].
#[derive(Debug, thiserror::Error)]
pub enum CreationError {
    /// Missing prevrandao for post-merge blockchain
    #[error("Missing prevrandao for post-merge blockchain")]
    MissingPrevrandao,
}

/// Helper type for a local Ethereum block for a given chain spec.
pub type EthLocalBlockForChainSpec<ChainSpecT> = EthLocalBlock<
    <ChainSpecT as RuntimeSpec>::RpcBlockConversionError,
    <ChainSpecT as RuntimeSpec>::BlockReceipt,
    ExecutionReceiptTypeConstructorForChainSpec<ChainSpecT>,
    <ChainSpecT as ChainHardfork>::Hardfork,
    <ChainSpecT as RuntimeSpec>::RpcReceiptConversionError,
    <ChainSpecT as ChainSpec>::SignedTransaction,
>;

/// A locally mined block, which contains complete information.
#[derive_where(Clone; SignedTransactionT)]
#[derive_where(Debug, PartialEq, Eq; BlockReceiptT, SignedTransactionT)]
pub struct EthLocalBlock<
    BlockConversionErrorT,
    BlockReceiptT: ReceiptTrait,
    ExecutionReceiptTypeConstructorT: ExecutionReceiptTypeConstructorBounds,
    HardforkT,
    ReceiptConversionErrorT,
    SignedTransactionT,
> {
    header: block::Header,
    transactions: Vec<SignedTransactionT>,
    transaction_receipts: Vec<Arc<BlockReceiptT>>,
    ommers: Vec<block::Header>,
    ommer_hashes: Vec<B256>,
    withdrawals: Option<Vec<Withdrawal>>,
    hash: B256,
    phantom: PhantomData<(
        BlockConversionErrorT,
        ExecutionReceiptTypeConstructorT,
        HardforkT,
        ReceiptConversionErrorT,
    )>,
}

impl<
        BlockConversionErrorT,
        BlockReceiptT: ReceiptTrait,
        ExecutionReceiptTypeConstructorT: ExecutionReceiptTypeConstructorBounds,
        HardforkT: Clone,
        ReceiptConversionErrorT,
        SignedTransactionT: Debug + ExecutableTransaction,
    >
    EthLocalBlock<
        BlockConversionErrorT,
        BlockReceiptT,
        ExecutionReceiptTypeConstructorT,
        HardforkT,
        ReceiptConversionErrorT,
        SignedTransactionT,
    >
{
    /// Constructs a new instance with the provided data.
    pub fn new(
        receipt_factory: impl ReceiptFactory<
            <ExecutionReceiptTypeConstructorT as TypeConstructor<FilterLog>>::Type,
            HardforkT,
            SignedTransactionT,
            Output = BlockReceiptT,
        >,
        hardfork: HardforkT,
        partial_header: PartialHeader,
        transactions: Vec<SignedTransactionT>,
        transaction_receipts: Vec<
            TransactionReceipt<
                <ExecutionReceiptTypeConstructorT as TypeConstructor<ExecutionLog>>::Type,
            >,
        >,
        ommers: Vec<Header>,
        withdrawals: Option<Vec<Withdrawal>>,
    ) -> Self {
        let ommer_hashes = ommers.iter().map(Header::hash).collect::<Vec<_>>();
        let transactions_root =
            trie::ordered_trie_root(transactions.iter().map(ExecutableTransaction::rlp_encoding));

        let header = Header::new(partial_header, transactions_root);

        let hash = header.hash();
        let transaction_receipts =
            map_transaction_receipt_logs::<ExecutionReceiptTypeConstructorT>(
                hash,
                header.number,
                transaction_receipts,
            )
            .zip(transactions.iter())
            .map(|(transaction_receipt, transaction)| {
                Arc::new(receipt_factory.create_receipt(
                    hardfork.clone(),
                    transaction,
                    transaction_receipt,
                    &hash,
                    header.number,
                ))
            })
            .collect();

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
    ) -> impl Iterator<Item = DetailedTransaction<'_, SignedTransactionT, Arc<BlockReceiptT>>> {
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
        BlockReceiptT: Debug + ReceiptTrait + alloy_rlp::Encodable,
        ExecutionReceiptTypeConstructorT: ExecutionReceiptTypeConstructorBounds,
        HardforkT,
        ReceiptConversionErrorT,
        SignedTransactionT: Debug + alloy_rlp::Encodable,
    >
    EthLocalBlock<
        BlockConversionErrorT,
        BlockReceiptT,
        ExecutionReceiptTypeConstructorT,
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
        BlockReceiptT: ReceiptTrait,
        ExecutionReceiptTypeConstructorT: ExecutionReceiptTypeConstructorBounds,
        HardforkT: Clone + Into<EvmSpecId>,
        ReceiptConversionErrorT,
        SignedTransactionT: Debug + ExecutableTransaction,
    >
    EthLocalBlock<
        BlockConversionErrorT,
        BlockReceiptT,
        ExecutionReceiptTypeConstructorT,
        HardforkT,
        ReceiptConversionErrorT,
        SignedTransactionT,
    >
{
    /// Constructs a block with the provided genesis state and options.
    pub fn with_genesis_state<HeaderConstantsT: EthHeaderConstants<Hardfork = HardforkT>>(
        genesis_diff: StateDiff,
        hardfork: HardforkT,
        base_fee_params: &BaseFeeParams<HardforkT>,
        options: GenesisBlockOptions<HardforkT>,
    ) -> Result<Self, CreationError>
    where
        HardforkT: Default,
    {
        let mut genesis_state = TrieState::default();
        genesis_state.commit(genesis_diff.clone().into());

        let evm_spec_id = hardfork.clone().into();
        if evm_spec_id >= EvmSpecId::MERGE && options.mix_hash.is_none() {
            return Err(CreationError::MissingPrevrandao);
        }

        let mut options = HeaderOverrides::<HardforkT>::from(options);
        options.state_root = Some(
            genesis_state
                .state_root()
                .expect("TrieState is guaranteed to successfully compute the state root"),
        );

        if options.timestamp.is_none() {
            options.timestamp = Some(
                SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .expect("Current time must be after unix epoch")
                    .as_secs(),
            );
        }

        // No ommers in the genesis block
        let ommers = Vec::new();

        let withdrawals = if evm_spec_id >= EvmSpecId::SHANGHAI {
            // Empty withdrawals for genesis block
            Some(Vec::new())
        } else {
            None
        };

        let partial_header = PartialHeader::new::<HeaderConstantsT>(
            hardfork.clone(),
            base_fee_params,
            options,
            None,
            &ommers,
            withdrawals.as_ref(),
        );

        Ok(Self::empty(hardfork, partial_header))
    }
}

impl<
        BlockConversionErrorT,
        BlockReceiptT: Debug + ReceiptTrait + alloy_rlp::Encodable,
        ExecutionReceiptTypeConstructorT: ExecutionReceiptTypeConstructorBounds,
        HardforkT,
        ReceiptConversionErrorT,
        SignedTransactionT: Debug + alloy_rlp::Encodable,
    > Block<SignedTransactionT>
    for EthLocalBlock<
        BlockConversionErrorT,
        BlockReceiptT,
        ExecutionReceiptTypeConstructorT,
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
        BlockReceiptT: ReceiptTrait + Debug + alloy_rlp::Encodable,
        ExecutionReceiptTypeConstructorT: ExecutionReceiptTypeConstructorBounds,
        HardforkT: Debug,
        ReceiptConversionErrorT,
        SignedTransactionT: Debug + alloy_rlp::Encodable,
    > BlockReceipts<Arc<BlockReceiptT>>
    for EthLocalBlock<
        BlockConversionErrorT,
        BlockReceiptT,
        ExecutionReceiptTypeConstructorT,
        HardforkT,
        ReceiptConversionErrorT,
        SignedTransactionT,
    >
{
    type Error = BlockchainError<BlockConversionErrorT, HardforkT, ReceiptConversionErrorT>;

    fn fetch_transaction_receipts(
        &self,
    ) -> Result<
        Vec<Arc<BlockReceiptT>>,
        BlockchainError<BlockConversionErrorT, HardforkT, ReceiptConversionErrorT>,
    > {
        Ok(self.transaction_receipts.clone())
    }
}

impl<
        BlockConversionErrorT,
        BlockReceiptT: ReceiptTrait,
        ExecutionReceiptTypeConstructorT: ExecutionReceiptTypeConstructorBounds,
        HardforkT: Into<EvmSpecId>,
        ReceiptConversionErrorT,
        SignedTransactionT: Debug + ExecutableTransaction,
    > EmptyBlock<HardforkT>
    for EthLocalBlock<
        BlockConversionErrorT,
        BlockReceiptT,
        ExecutionReceiptTypeConstructorT,
        HardforkT,
        ReceiptConversionErrorT,
        SignedTransactionT,
    >
{
    fn empty(hardfork: HardforkT, partial_header: PartialHeader) -> Self {
        let withdrawals = if hardfork.into() >= EvmSpecId::SHANGHAI {
            Some(Vec::new())
        } else {
            None
        };

        let header = Header::new(partial_header, KECCAK_EMPTY);
        let hash = header.hash();

        Self {
            header,
            transactions: Vec::new(),
            transaction_receipts: Vec::new(),
            ommers: Vec::new(),
            ommer_hashes: Vec::new(),
            withdrawals,
            hash,
            phantom: PhantomData,
        }
    }
}

impl<
        BlockConversionErrorT,
        BlockReceiptT: ReceiptTrait,
        ExecutionReceiptTypeConstructorT: ExecutionReceiptTypeConstructorBounds,
        HardforkT,
        ReceiptConversionErrorT,
        SignedTransactionT: Debug + ExecutableTransaction + alloy_rlp::Encodable,
    > LocalBlock<Arc<BlockReceiptT>>
    for EthLocalBlock<
        BlockConversionErrorT,
        BlockReceiptT,
        ExecutionReceiptTypeConstructorT,
        HardforkT,
        ReceiptConversionErrorT,
        SignedTransactionT,
    >
{
    fn transaction_receipts(&self) -> &[Arc<BlockReceiptT>] {
        &self.transaction_receipts
    }
}

impl<
        BlockConversionErrorT,
        BlockReceiptT: Debug + ReceiptTrait + alloy_rlp::Encodable,
        ExecutionReceiptTypeConstructorT: ExecutionReceiptTypeConstructorBounds,
        HardforkT,
        ReceiptConversionErrorT,
        SignedTransactionT: Debug + alloy_rlp::Encodable,
    > alloy_rlp::Encodable
    for EthLocalBlock<
        BlockConversionErrorT,
        BlockReceiptT,
        ExecutionReceiptTypeConstructorT,
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

/// Maps the logs of the transaction receipts from [`ExecutionLog`] to
/// [`FilterLog`].
fn map_transaction_receipt_logs<
    ExecutionReceiptTypeConstructorT: ExecutionReceiptTypeConstructorBounds,
>(
    block_hash: B256,
    block_number: u64,
    receipts: Vec<
        TransactionReceipt<
            <ExecutionReceiptTypeConstructorT as TypeConstructor<ExecutionLog>>::Type,
        >,
    >,
) -> impl Iterator<
    Item = TransactionReceipt<
        <ExecutionReceiptTypeConstructorT as TypeConstructor<FilterLog>>::Type,
    >,
> {
    let mut log_index = 0;

    receipts
        .into_iter()
        .enumerate()
        .map(move |(transaction_index, receipt)| {
            let transaction_index = transaction_index as u64;
            let transaction_hash = receipt.transaction_hash;

            receipt.map_logs(|log| {
                FilterLog {
                    inner: FullBlockLog {
                        inner: ReceiptLog {
                            inner: log,
                            transaction_hash,
                        },
                        block_hash,
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
            })
        })
}
