use std::sync::{Arc, OnceLock};

use derive_where::derive_where;
use edr_eth::{
    block::{BlobGas, Header},
    receipt::BlockReceipt,
    transaction::{self, SignedTransaction},
    withdrawal::Withdrawal,
    B256, U256,
};
use edr_rpc_eth::{client::EthRpcClient, TransactionConversionError};
use tokio::runtime;

use crate::{
    blockchain::{BlockchainError, ForkedBlockchainError},
    chain_spec::{ChainSpec, L1ChainSpec, SyncChainSpec},
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
    for Arc<dyn SyncBlock<ChainSpecT, Error = BlockchainError>>
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

    /// Returns the total difficulty of the chain until this block for finalised
    /// blocks. For pending blocks, returns `None`.
    fn total_difficulty(&self) -> Option<&U256>;
}

impl<TransactionT> EthRpcBlock for edr_rpc_eth::Block<TransactionT> {
    fn state_root(&self) -> &B256 {
        &self.state_root
    }

    fn total_difficulty(&self) -> Option<&U256> {
        self.total_difficulty.as_ref()
    }
}

/// Trait for types that can be converted into a remote Ethereum block.
pub trait IntoRemoteBlock<ChainSpecT: ChainSpec> {
    /// Converts the instance into a `RemoteBlock` with the provided JSON-RPC
    /// client and tokio runtime.
    fn into_remote_block(
        self,
        rpc_client: Arc<EthRpcClient<ChainSpecT>>,
        runtime: runtime::Handle,
    ) -> Result<RemoteBlock<ChainSpecT>, CreationError>;
}

impl IntoRemoteBlock<L1ChainSpec> for edr_rpc_eth::Block<edr_rpc_eth::Transaction> {
    fn into_remote_block(
        self,
        rpc_client: Arc<EthRpcClient<L1ChainSpec>>,
        runtime: runtime::Handle,
    ) -> Result<RemoteBlock<L1ChainSpec>, CreationError> {
        let header = Header {
            parent_hash: self.parent_hash,
            ommers_hash: self.sha3_uncles,
            beneficiary: self.miner.ok_or(CreationError::MissingMiner)?,
            state_root: self.state_root,
            transactions_root: self.transactions_root,
            receipts_root: self.receipts_root,
            logs_bloom: self.logs_bloom,
            difficulty: self.difficulty,
            number: self.number.ok_or(CreationError::MissingNumber)?,
            gas_limit: self.gas_limit,
            gas_used: self.gas_used,
            timestamp: self.timestamp,
            extra_data: self.extra_data,
            // TODO don't accept remote blocks with missing mix hash,
            // see https://github.com/NomicFoundation/edr/issues/518
            mix_hash: self.mix_hash.unwrap_or_default(),
            nonce: self.nonce.ok_or(CreationError::MissingNonce)?,
            base_fee_per_gas: self.base_fee_per_gas,
            withdrawals_root: self.withdrawals_root,
            blob_gas: self.blob_gas_used.and_then(|gas_used| {
                self.excess_blob_gas.map(|excess_gas| BlobGas {
                    gas_used,
                    excess_gas,
                })
            }),
            parent_beacon_block_root: self.parent_beacon_block_root,
        };

        let transactions = self
            .transactions
            .into_iter()
            .map(transaction::Signed::try_from)
            .collect::<Result<Vec<_>, _>>()?;

        let hash = self.hash.ok_or(CreationError::MissingHash)?;

        Ok(RemoteBlock {
            header,
            transactions,
            receipts: OnceLock::new(),
            ommer_hashes: self.uncles,
            withdrawals: self.withdrawals,
            hash,
            rpc_client,
            size: self.size,
            runtime,
        })
    }
}
