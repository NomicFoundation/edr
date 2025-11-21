use core::{fmt::Debug, marker::PhantomData};
use std::sync::{Arc, OnceLock};

use derive_where::derive_where;
use edr_block_api::{sync::SyncBlock, Block, EthBlockData, FetchBlockReceipts};
use edr_block_header::{BlockHeader, Withdrawal};
use edr_chain_spec::ExecutableTransaction;
use edr_chain_spec_rpc::RpcBlockChainSpec;
use edr_primitives::B256;
use edr_receipt::ReceiptTrait;
use edr_rpc_eth::client::{EthRpcClient, RpcClientError};
use edr_utils::CastArcFrom;
use tokio::runtime;

/// Error that occurs when trying to convert the JSON-RPC `Block` type.
#[derive(Debug, thiserror::Error)]
pub enum RpcBlockConversionError<TransactionConversionErrorT> {
    /// Missing hash
    #[error("Missing hash")]
    MissingHash,
    /// Missing miner
    #[error("Missing miner")]
    MissingMiner,
    /// Missing mix hash
    #[error("Missing mix hash")]
    MissingMixHash,
    /// Missing nonce
    #[error("Missing nonce")]
    MissingNonce,
    /// Missing number
    #[error("Missing number")]
    MissingNumber,
    /// Transaction conversion error
    #[error(transparent)]
    TransactionConversionError(TransactionConversionErrorT),
}

/// A remote block, which lazily loads receipts.
#[derive_where(Clone; SignedTransactionT)]
#[derive_where(Debug; BlockReceiptT, SignedTransactionT)]
pub struct RemoteBlock<
    BlockReceiptT,
    FetchReceiptErrorT,
    RpcBlockChainSpecT: RpcBlockChainSpec,
    RpcReceiptT: serde::de::DeserializeOwned + serde::Serialize,
    RpcTransactionT: serde::de::DeserializeOwned + serde::Serialize,
    SignedTransactionT,
> {
    header: BlockHeader,
    transactions: Vec<SignedTransactionT>,
    /// The receipts of the block's transactions
    receipts: OnceLock<Vec<Arc<BlockReceiptT>>>,
    /// The hashes of the block's ommers
    ommer_hashes: Vec<B256>,
    /// The staking withdrawals
    withdrawals: Option<Vec<Withdrawal>>,
    /// The block's hash
    hash: B256,
    /// The length of the RLP encoding of this block in bytes
    size: u64,
    // The RPC client is needed to lazily fetch receipts
    rpc_client: Arc<EthRpcClient<RpcBlockChainSpecT, RpcReceiptT, RpcTransactionT>>,
    runtime: runtime::Handle,
    _phantom: PhantomData<fn() -> FetchReceiptErrorT>,
}

impl<
        BlockReceiptT,
        FetchReceiptErrorT,
        RpcBlockChainSpecT: RpcBlockChainSpec<RpcBlock<RpcTransactionT>: TryInto<EthBlockData<SignedTransactionT>>>,
        RpcReceiptT: serde::de::DeserializeOwned + serde::Serialize,
        RpcTransactionT: serde::de::DeserializeOwned + serde::Serialize,
        SignedTransactionT,
    >
    RemoteBlock<
        BlockReceiptT,
        FetchReceiptErrorT,
        RpcBlockChainSpecT,
        RpcReceiptT,
        RpcTransactionT,
        SignedTransactionT,
    >
{
    /// Tries to construct a new instance from a JSON-RPC block.
    pub fn new(
        block: RpcBlockChainSpecT::RpcBlock<RpcTransactionT>,
        rpc_client: Arc<EthRpcClient<RpcBlockChainSpecT, RpcReceiptT, RpcTransactionT>>,
        runtime: runtime::Handle,
    ) -> Result<
        Self,
        <RpcBlockChainSpecT::RpcBlock<RpcTransactionT> as TryInto<
            EthBlockData<SignedTransactionT>,
        >>::Error,
    > {
        let block: EthBlockData<SignedTransactionT> = block.try_into()?;

        Ok(Self {
            header: block.header,
            transactions: block.transactions,
            receipts: OnceLock::new(),
            ommer_hashes: block.ommer_hashes,
            withdrawals: block.withdrawals,
            hash: block.hash,
            size: block.rlp_size,
            rpc_client,
            runtime,
            _phantom: PhantomData,
        })
    }
}

impl<
        BlockReceiptT: Debug,
        FetchReceiptErrorT,
        RpcBlockT: RpcBlockChainSpec,
        RpcReceiptT: serde::de::DeserializeOwned + serde::Serialize,
        RpcTransactionT: serde::de::DeserializeOwned + serde::Serialize,
        SignedTransactionT: Debug,
    > Block<SignedTransactionT>
    for RemoteBlock<
        BlockReceiptT,
        FetchReceiptErrorT,
        RpcBlockT,
        RpcReceiptT,
        RpcTransactionT,
        SignedTransactionT,
    >
{
    fn block_hash(&self) -> &B256 {
        &self.hash
    }

    fn block_header(&self) -> &BlockHeader {
        &self.header
    }

    fn ommer_hashes(&self) -> &[B256] {
        self.ommer_hashes.as_slice()
    }

    fn rlp_size(&self) -> u64 {
        self.size
    }

    fn transactions(&self) -> &[SignedTransactionT] {
        &self.transactions
    }

    fn withdrawals(&self) -> Option<&[Withdrawal]> {
        self.withdrawals.as_deref()
    }
}

/// An error that occurs when fetching a remote receipt.
#[derive(Debug, thiserror::Error)]
pub enum FetchRemoteReceiptError<RpcReceiptConversionErrorT> {
    /// Error converting a receipt
    #[error(transparent)]
    Conversion(RpcReceiptConversionErrorT),
    /// Missing transaction receipts for a remote block
    #[error("Missing receipts for block {block_hash}")]
    MissingReceipts {
        /// The block hash
        block_hash: B256,
    },
    /// RPC client error
    #[error(transparent)]
    RpcClient(#[from] RpcClientError),
}

impl<
        BlockReceiptT: Debug + ReceiptTrait + TryFrom<RpcReceiptT, Error = RpcReceiptConversionErrorT>,
        FetchReceiptErrorT: From<FetchRemoteReceiptError<RpcReceiptConversionErrorT>>,
        RpcBlockChainSpecT: RpcBlockChainSpec,
        RpcReceiptConversionErrorT,
        RpcReceiptT: serde::de::DeserializeOwned + serde::Serialize,
        RpcTransactionT: serde::de::DeserializeOwned + serde::Serialize,
        SignedTransactionT: Debug + ExecutableTransaction,
    > FetchBlockReceipts<Arc<BlockReceiptT>>
    for RemoteBlock<
        BlockReceiptT,
        FetchReceiptErrorT,
        RpcBlockChainSpecT,
        RpcReceiptT,
        RpcTransactionT,
        SignedTransactionT,
    >
{
    type Error = FetchReceiptErrorT;

    fn fetch_transaction_receipts(&self) -> Result<Vec<Arc<BlockReceiptT>>, FetchReceiptErrorT> {
        if let Some(receipts) = self.receipts.get() {
            return Ok(receipts.clone());
        }

        let receipts: Vec<Arc<BlockReceiptT>> = tokio::task::block_in_place(|| {
            self.runtime.block_on(
                self.rpc_client.get_transaction_receipts(
                    self.transactions
                        .iter()
                        .map(SignedTransactionT::transaction_hash),
                ),
            )
        })
        .map_err(FetchRemoteReceiptError::RpcClient)?
        .ok_or_else(|| FetchRemoteReceiptError::MissingReceipts {
            block_hash: *self.block_hash(),
        })?
        .into_iter()
        .map(|receipt| receipt.try_into().map(Arc::new))
        .collect::<Result<_, _>>()
        .map_err(FetchRemoteReceiptError::Conversion)?;

        self.receipts
            .set(receipts.clone())
            .expect("We checked that receipts are not set");

        Ok(receipts)
    }
}

impl<
        BlockReceiptT: 'static
            + Debug
            + ReceiptTrait
            + Send
            + Sync
            + TryFrom<RpcReceiptT, Error = RpcReceiptConversionErrorT>,
        FetchReceiptErrorT: 'static + From<FetchRemoteReceiptError<RpcReceiptConversionErrorT>>,
        RpcBlockChainSpecT: 'static + RpcBlockChainSpec,
        RpcReceiptConversionErrorT: 'static,
        RpcReceiptT: 'static + serde::de::DeserializeOwned + serde::Serialize,
        RpcTransactionT: 'static + serde::de::DeserializeOwned + serde::Serialize,
        SignedTransactionT: 'static + Debug + ExecutableTransaction + Send + Sync,
    >
    CastArcFrom<
        RemoteBlock<
            BlockReceiptT,
            FetchReceiptErrorT,
            RpcBlockChainSpecT,
            RpcReceiptT,
            RpcTransactionT,
            SignedTransactionT,
        >,
    > for dyn SyncBlock<Arc<BlockReceiptT>, SignedTransactionT, Error = FetchReceiptErrorT>
{
    fn cast_arc_from(
        value: Arc<
            RemoteBlock<
                BlockReceiptT,
                FetchReceiptErrorT,
                RpcBlockChainSpecT,
                RpcReceiptT,
                RpcTransactionT,
                SignedTransactionT,
            >,
        >,
    ) -> Arc<Self> {
        value
    }
}
