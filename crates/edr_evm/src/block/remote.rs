use std::sync::{Arc, OnceLock};

use derive_where::derive_where;
use edr_eth::{
    block::Header, receipt::BlockReceipt, transaction::SignedTransaction as _,
    withdrawal::Withdrawal, B256, U256,
};
use edr_rpc_eth::client::EthRpcClient;
use tokio::runtime;

use crate::{
    blockchain::{BlockchainError, ForkedBlockchainError},
    chain_spec::{ChainSpec, SyncChainSpec},
    Block, EthBlockData, SyncBlock,
};

/// Error that occurs when trying to convert the JSON-RPC `Block` type.
#[derive(thiserror::Error)]
#[derive_where(Debug; ChainSpecT::RpcTransactionConversionError)]
pub enum ConversionError<ChainSpecT: ChainSpec> {
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
    TransactionConversionError(ChainSpecT::RpcTransactionConversionError),
}

/// A remote block, which lazily loads receipts.
#[derive_where(Clone, Debug; ChainSpecT::Transaction)]
pub struct RemoteBlock<ChainSpecT: ChainSpec> {
    header: Header,
    transactions: Vec<ChainSpecT::Transaction>,
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
    /// Tries to construct a new instance from a JSON-RPC block.
    pub fn new(
        block: ChainSpecT::RpcBlock<ChainSpecT::RpcTransaction>,
        rpc_client: Arc<EthRpcClient<ChainSpecT>>,
        runtime: runtime::Handle,
    ) -> Result<Self, ChainSpecT::RpcBlockConversionError> {
        let block = TryInto::<EthBlockData<ChainSpecT>>::try_into(block)?;

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
        })
    }
}

impl<ChainSpecT: ChainSpec> Block<ChainSpecT> for RemoteBlock<ChainSpecT> {
    type Error = BlockchainError<ChainSpecT>;

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

    fn transactions(&self) -> &[ChainSpecT::Transaction] {
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
                        .map(ChainSpecT::Transaction::transaction_hash),
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

impl<ChainSpecT> From<RemoteBlock<ChainSpecT>>
    for Arc<dyn SyncBlock<ChainSpecT, Error = BlockchainError<ChainSpecT>>>
where
    ChainSpecT: SyncChainSpec,
{
    fn from(value: RemoteBlock<ChainSpecT>) -> Self {
        Arc::new(value)
    }
}

/// Trait that provides access to the state root and total difficulty of an
/// Ethereum-based block.
pub trait EthRpcBlock {
    /// Returns the root of the block's state trie.
    fn state_root(&self) -> &B256;

    /// Returns the block's timestamp.
    fn timestamp(&self) -> u64;

    /// Returns the total difficulty of the chain until this block for finalised
    /// blocks. For pending blocks, returns `None`.
    fn total_difficulty(&self) -> Option<&U256>;
}

impl<TransactionT> EthRpcBlock for edr_rpc_eth::Block<TransactionT> {
    fn state_root(&self) -> &B256 {
        &self.state_root
    }

    fn timestamp(&self) -> u64 {
        self.timestamp
    }

    fn total_difficulty(&self) -> Option<&U256> {
        self.total_difficulty.as_ref()
    }
}
