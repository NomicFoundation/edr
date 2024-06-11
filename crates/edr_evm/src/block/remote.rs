use std::sync::{Arc, OnceLock};

use edr_eth::{
    block::{BlobGas, Header},
    receipt::BlockReceipt,
    transaction::{self, Transaction},
    withdrawal::Withdrawal,
    B256,
};
use edr_rpc_eth::{client::EthRpcClient, spec::EthRpcSpec, TransactionConversionError};
use tokio::runtime;

use crate::{
    blockchain::{BlockchainError, ForkedBlockchainError},
    chain_spec::ChainSpec,
    Block, SyncBlock,
};

/// Error that occurs when trying to convert the JSON-RPC `Block` type.
#[derive(Debug, thiserror::Error)]
pub enum CreationError {
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
    #[error("Missing numbeer")]
    MissingNumber,
    /// Transaction conversion error
    #[error(transparent)]
    TransactionConversionError(#[from] TransactionConversionError),
}

/// A remote block, which lazily loads receipts.
#[derive(Clone, Debug)]
pub struct RemoteBlock<ChainSpecT: ChainSpec> {
    header: Header,
    transactions: Vec<ChainSpecT::SignedTransaction>,
    /// The receipts of the block's transactions
    receipts: OnceLock<Vec<Arc<BlockReceipt>>>,
    /// The hashes of the block's ommers
    ommer_hashes: Vec<B256>,
    /// The staking withdrawals
    withdrawals: Option<Vec<Withdrawal>>,
    /// The block's hash
    hash: B256,
    /// The length of the RLP encoding of this block in bytes
    size: u64,
    // The RPC client is needed to lazily fetch receipts
    rpc_client: Arc<EthRpcClient<ChainSpecT>>,
    runtime: runtime::Handle,
}

impl<ChainSpecT: ChainSpec> RemoteBlock<ChainSpecT> {
    /// Constructs a new instance with the provided JSON-RPC block and client.
    pub fn new(
        block: ChainSpecT::RpcBlock<ChainSpecT::RpcTransaction>,
        rpc_client: Arc<EthRpcClient<ChainSpecT>>,
        runtime: runtime::Handle,
    ) -> Result<Self, CreationError> {
        let header = Header::try_from(&block)?

        let transactions = block
            .transactions
            .into_iter()
            .map(ChainSpecT::SignedTransaction::try_from)
            .collect::<Result<Vec<_>, _>>()?;

        let hash = block.hash.ok_or(CreationError::MissingHash)?;

        Ok(Self {
            header,
            transactions,
            receipts: OnceLock::new(),
            ommer_hashes: block.uncles,
            withdrawals: block.withdrawals,
            hash,
            rpc_client,
            size: block.size,
            runtime,
        })
    }
}

impl<ChainSpecT: ChainSpec> Block<ChainSpecT> for RemoteBlock<ChainSpecT> {
    type Error = BlockchainError;

    fn hash(&self) -> &B256 {
        &self.hash
    }

    fn header(&self) -> &Header {
        &self.header
    }

    fn ommer_hashes(&self) -> &[B256] {
        self.ommer_hashes.as_slice()
    }

    fn rlp_size(&self) -> u64 {
        self.size
    }

    fn transactions(&self) -> &[ChainSpecT::SignedTransaction] {
        &self.transactions
    }

    fn transaction_receipts(&self) -> Result<Vec<Arc<BlockReceipt>>, Self::Error> {
        if let Some(receipts) = self.receipts.get() {
            return Ok(receipts.clone());
        }

        let receipts: Vec<Arc<BlockReceipt>> = tokio::task::block_in_place(|| {
            self.runtime.block_on(
                self.rpc_client.get_transaction_receipts(
                    self.transactions
                        .iter()
                        .map(ChainSpecT::SignedTransaction::transaction_hash),
                ),
            )
        })
        .map_err(ForkedBlockchainError::RpcClient)?
        .ok_or_else(|| ForkedBlockchainError::MissingReceipts {
            block_hash: *self.hash(),
        })?
        .into_iter()
        .map(Arc::new)
        .collect();

        self.receipts
            .set(receipts.clone())
            .expect("We checked that receipts are not set");

        Ok(receipts)
    }

    fn withdrawals(&self) -> Option<&[Withdrawal]> {
        self.withdrawals.as_deref()
    }
}

impl<ChainSpecT> From<RemoteBlock<ChainSpecT>> for Arc<dyn SyncBlock<ChainSpecT, Error = BlockchainError>>
where
    ChainSpecT: ChainSpec + Send + Sync,
    ChainSpecT::SignedTransaction: Send + Sync, {
    fn from(value: RemoteBlock<ChainSpecT>) -> Self {
        Arc::new(value)
    }
}
