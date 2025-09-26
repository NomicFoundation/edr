use std::sync::{Arc, OnceLock};

use derive_where::derive_where;
use edr_block_header::{BlockHeader, Withdrawal};
use edr_chain_l1::rpc::block::L1RpcBlock;
use edr_evm_spec::ExecutableTransaction as _;
use edr_primitives::{B256, U256};
use edr_rpc_eth::client::EthRpcClient;
use tokio::runtime;

use crate::{
    block::BlockReceipts,
    blockchain::{BlockchainErrorForChainSpec, ForkedBlockchainError},
    spec::RuntimeSpec,
    Block, EthBlockData,
};

/// Error that occurs when trying to convert the JSON-RPC `Block` type.
#[derive(Debug, thiserror::Error)]
pub enum ConversionError<TransactionConversionErrorT> {
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
    TransactionConversionError(TransactionConversionErrorT),
}

/// A remote block, which lazily loads receipts.
#[derive_where(Clone; ChainSpecT::SignedTransaction)]
#[derive_where(Debug; ChainSpecT::SignedTransaction, ChainSpecT::BlockReceipt)]
pub struct RemoteBlock<ChainSpecT: RuntimeSpec> {
    header: BlockHeader,
    transactions: Vec<ChainSpecT::SignedTransaction>,
    /// The receipts of the block's transactions
    receipts: OnceLock<Vec<Arc<ChainSpecT::BlockReceipt>>>,
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

impl<ChainSpecT: RuntimeSpec> RemoteBlock<ChainSpecT> {
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

impl<ChainSpecT: RuntimeSpec> Block<ChainSpecT::SignedTransaction> for RemoteBlock<ChainSpecT> {
    fn block_hash(&self) -> &B256 {
        &self.hash
    }

    fn header(&self) -> &BlockHeader {
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

    fn withdrawals(&self) -> Option<&[Withdrawal]> {
        self.withdrawals.as_deref()
    }
}

impl<ChainSpecT: RuntimeSpec> BlockReceipts<Arc<ChainSpecT::BlockReceipt>>
    for RemoteBlock<ChainSpecT>
{
    type Error = BlockchainErrorForChainSpec<ChainSpecT>;

    fn fetch_transaction_receipts(
        &self,
    ) -> Result<Vec<Arc<ChainSpecT::BlockReceipt>>, Self::Error> {
        if let Some(receipts) = self.receipts.get() {
            return Ok(receipts.clone());
        }

        let receipts: Vec<Arc<ChainSpecT::BlockReceipt>> = tokio::task::block_in_place(|| {
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
            block_hash: *self.block_hash(),
        })?
        .into_iter()
        .map(|receipt| receipt.try_into().map(Arc::new))
        .collect::<Result<_, _>>()
        .map_err(ForkedBlockchainError::ReceiptConversion)?;

        self.receipts
            .set(receipts.clone())
            .expect("We checked that receipts are not set");

        Ok(receipts)
    }
}
