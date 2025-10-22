use core::{fmt::Debug, marker::PhantomData};
use std::{
    sync::Arc,
    time::{SystemTime, UNIX_EPOCH},
};

use alloy_rlp::Encodable as _;
use derive_where::derive_where;
use edr_block_api::{
    sync::SyncBlock, Block, BlockReceipts, EmptyBlock, GenesisBlockOptions, LocalBlock,
};
use edr_block_header::{BlockConfig, BlockHeader, HeaderOverrides, PartialHeader, Withdrawal};
use edr_chain_spec::{EvmSpecId, ExecutableTransaction};
use edr_primitives::{B256, KECCAK_EMPTY};
use edr_receipt::{
    log::{ExecutionLog, FilterLog, FullBlockLog, ReceiptLog},
    ExecutionReceiptChainSpec, MapReceiptLogs, ReceiptTrait, TransactionReceipt,
};
use edr_receipt_spec::ReceiptConstructor;
use edr_state_api::{StateCommit as _, StateDebug as _, StateDiff};
use edr_state_persistent_trie::PersistentStateTrie;
use edr_transaction::TransactionAndReceipt;
use edr_trie::ordered_trie_root;
use edr_utils::CastArcFrom;
use itertools::izip;

/// A locally mined block, which contains complete information.
#[derive_where(Clone; SignedTransactionT)]
#[derive_where(Debug, PartialEq, Eq; BlockReceiptT, SignedTransactionT)]
pub struct EthLocalBlock<
    BlockReceiptT: ReceiptTrait,
    FetchReceiptErrorT,
    HardforkT,
    SignedTransactionT,
> {
    header: BlockHeader,
    transactions: Vec<SignedTransactionT>,
    transaction_receipts: Vec<Arc<BlockReceiptT>>,
    ommers: Vec<BlockHeader>,
    ommer_hashes: Vec<B256>,
    withdrawals: Option<Vec<Withdrawal>>,
    hash: B256,
    _phantom: PhantomData<fn() -> (FetchReceiptErrorT, HardforkT)>,
}

impl<
        BlockReceiptT: ReceiptConstructor<
                Context = ContextT,
                Hardfork = HardforkT,
                SignedTransaction = SignedTransactionT,
            > + ReceiptTrait,
        ContextT,
        FetchReceiptErrorT,
        HardforkT: Clone,
        SignedTransactionT: ExecutableTransaction,
    > EthLocalBlock<BlockReceiptT, FetchReceiptErrorT, HardforkT, SignedTransactionT>
{
    /// Constructs a new instance with the provided data.
    pub fn new<
        ExecutionReceiptChainSpecT: ExecutionReceiptChainSpec<
                ExecutionReceipt<ExecutionLog>: MapReceiptLogs<
                    ExecutionLog,
                    FilterLog,
                    ExecutionReceiptChainSpecT::ExecutionReceipt<FilterLog>,
                >,
            > + ExecutionReceiptChainSpec<ExecutionReceipt<FilterLog> = BlockReceiptT::ExecutionReceipt>,
    >(
        context: &ContextT,
        hardfork: HardforkT,
        partial_header: PartialHeader,
        transactions: Vec<SignedTransactionT>,
        transaction_receipts: Vec<
            TransactionReceipt<ExecutionReceiptChainSpecT::ExecutionReceipt<ExecutionLog>>,
        >,
        ommers: Vec<BlockHeader>,
        withdrawals: Option<Vec<Withdrawal>>,
    ) -> Self {
        let ommer_hashes = ommers.iter().map(BlockHeader::hash).collect::<Vec<_>>();
        let transactions_root =
            ordered_trie_root(transactions.iter().map(ExecutableTransaction::rlp_encoding));

        let header = BlockHeader::new(partial_header, transactions_root);

        let hash = header.hash();
        let transaction_receipts = map_transaction_receipt_logs::<ExecutionReceiptChainSpecT>(
            hash,
            header.number,
            transaction_receipts,
        )
        .zip(transactions.iter())
        .map(|(transaction_receipt, transaction)| {
            Arc::new(BlockReceiptT::new_receipt(
                context,
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
            _phantom: PhantomData,
        }
    }
}

impl<
        BlockReceiptT: ReceiptTrait,
        FetchReceiptErrorT,
        HardforkT: Clone,
        SignedTransactionT: Debug + ExecutableTransaction,
    > EthLocalBlock<BlockReceiptT, FetchReceiptErrorT, HardforkT, SignedTransactionT>
{
    /// Retrieves the block's transactions along with their receipts.
    pub fn transactions_with_receipt(
        &self,
    ) -> impl Iterator<Item = TransactionAndReceipt<'_, SignedTransactionT, Arc<BlockReceiptT>>>
    {
        izip!(self.transactions.iter(), self.transaction_receipts.iter()).map(
            |(transaction, receipt)| TransactionAndReceipt {
                transaction,
                receipt,
            },
        )
    }
}

impl<
        BlockReceiptT: Debug + ReceiptTrait + alloy_rlp::Encodable,
        FetchReceiptErrorT,
        HardforkT,
        SignedTransactionT: Debug + alloy_rlp::Encodable,
    > EthLocalBlock<BlockReceiptT, FetchReceiptErrorT, HardforkT, SignedTransactionT>
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

/// An error that occurs upon creation of an [`EthLocalBlock`].
#[derive(Debug, thiserror::Error)]
pub enum LocalBlockCreationError {
    /// Missing prevrandao for post-merge blockchain
    #[error("Missing prevrandao for post-merge blockchain")]
    MissingPrevrandao,
}

impl<
        BlockReceiptT: ReceiptTrait,
        FetchReceiptErrorT,
        HardforkT: Clone + Into<EvmSpecId> + PartialOrd,
        SignedTransactionT: Debug + ExecutableTransaction,
    > EthLocalBlock<BlockReceiptT, FetchReceiptErrorT, HardforkT, SignedTransactionT>
{
    /// Constructs a block with the provided genesis state and options.
    pub fn with_genesis_state(
        genesis_diff: StateDiff,
        block_config: BlockConfig<'_, HardforkT>,
        options: GenesisBlockOptions<HardforkT>,
    ) -> Result<Self, LocalBlockCreationError> {
        let mut genesis_state = PersistentStateTrie::default();
        genesis_state.commit(genesis_diff.clone().into());

        let evm_spec_id = block_config.hardfork.clone().into();
        if evm_spec_id >= EvmSpecId::MERGE && options.mix_hash.is_none() {
            return Err(LocalBlockCreationError::MissingPrevrandao);
        }

        let mut options = HeaderOverrides::from(options);
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
        let hardfork = block_config.hardfork.clone();

        let partial_header =
            PartialHeader::new(block_config, options, None, &ommers, withdrawals.as_ref());

        Ok(Self::empty(hardfork, partial_header))
    }
}

impl<
        BlockReceiptT: Debug + ReceiptTrait + alloy_rlp::Encodable,
        FetchReceiptErrorT,
        HardforkT,
        SignedTransactionT: Debug + alloy_rlp::Encodable,
    > Block<SignedTransactionT>
    for EthLocalBlock<BlockReceiptT, FetchReceiptErrorT, HardforkT, SignedTransactionT>
{
    fn block_hash(&self) -> &B256 {
        &self.hash
    }

    fn block_header(&self) -> &BlockHeader {
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
        BlockReceiptT: ReceiptTrait + Debug + alloy_rlp::Encodable,
        FetchReceiptErrorT,
        HardforkT: Debug,
        SignedTransactionT: Debug + alloy_rlp::Encodable,
    > BlockReceipts<Arc<BlockReceiptT>>
    for EthLocalBlock<BlockReceiptT, FetchReceiptErrorT, HardforkT, SignedTransactionT>
{
    type Error = FetchReceiptErrorT;

    fn fetch_transaction_receipts(&self) -> Result<Vec<Arc<BlockReceiptT>>, FetchReceiptErrorT> {
        Ok(self.transaction_receipts.clone())
    }
}

impl<
        BlockReceiptT: 'static + Debug + ReceiptTrait + Send + Sync + alloy_rlp::Encodable,
        FetchReceiptErrorT: 'static,
        HardforkT: 'static + Debug,
        SignedTransactionT: 'static + Debug + Send + Sync + alloy_rlp::Encodable,
    > CastArcFrom<EthLocalBlock<BlockReceiptT, FetchReceiptErrorT, HardforkT, SignedTransactionT>>
    for dyn SyncBlock<Arc<BlockReceiptT>, SignedTransactionT, Error = FetchReceiptErrorT>
{
    fn cast_arc_from(
        value: Arc<EthLocalBlock<BlockReceiptT, FetchReceiptErrorT, HardforkT, SignedTransactionT>>,
    ) -> Arc<Self> {
        value
    }
}

impl<
        BlockReceiptT: ReceiptTrait,
        FetchReceiptErrorT,
        HardforkT: Into<EvmSpecId>,
        SignedTransactionT: Debug + ExecutableTransaction,
    > EmptyBlock<HardforkT>
    for EthLocalBlock<BlockReceiptT, FetchReceiptErrorT, HardforkT, SignedTransactionT>
{
    fn empty(hardfork: HardforkT, partial_header: PartialHeader) -> Self {
        let withdrawals = if hardfork.into() >= EvmSpecId::SHANGHAI {
            Some(Vec::new())
        } else {
            None
        };

        let header = BlockHeader::new(partial_header, KECCAK_EMPTY);
        let hash = header.hash();

        Self {
            header,
            transactions: Vec::new(),
            transaction_receipts: Vec::new(),
            ommers: Vec::new(),
            ommer_hashes: Vec::new(),
            withdrawals,
            hash,
            _phantom: PhantomData,
        }
    }
}

impl<
        BlockReceiptT: ReceiptTrait,
        FetchReceiptErrorT,
        HardforkT,
        SignedTransactionT: Debug + ExecutableTransaction + alloy_rlp::Encodable,
    > LocalBlock<Arc<BlockReceiptT>>
    for EthLocalBlock<BlockReceiptT, FetchReceiptErrorT, HardforkT, SignedTransactionT>
{
    fn transaction_receipts(&self) -> &[Arc<BlockReceiptT>] {
        &self.transaction_receipts
    }
}

impl<
        BlockReceiptT: Debug + ReceiptTrait + alloy_rlp::Encodable,
        FetchReceiptErrorT,
        HardforkT,
        SignedTransactionT: Debug + alloy_rlp::Encodable,
    > alloy_rlp::Encodable
    for EthLocalBlock<BlockReceiptT, FetchReceiptErrorT, HardforkT, SignedTransactionT>
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
    ExecutionReceiptT: ExecutionReceiptChainSpec<
        ExecutionReceipt<ExecutionLog>: MapReceiptLogs<
            ExecutionLog,
            FilterLog,
            ExecutionReceiptT::ExecutionReceipt<FilterLog>,
        >,
    >,
>(
    block_hash: B256,
    block_number: u64,
    receipts: Vec<TransactionReceipt<ExecutionReceiptT::ExecutionReceipt<ExecutionLog>>>,
) -> impl Iterator<Item = TransactionReceipt<ExecutionReceiptT::ExecutionReceipt<FilterLog>>> {
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
