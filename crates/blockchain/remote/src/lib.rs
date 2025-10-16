use core::fmt::Debug;
use std::sync::Arc;

use async_rwlock::{RwLock, RwLockUpgradableReadGuard};
use edr_block_api::{Block, EthBlockData};
use edr_block_remote::RemoteBlock;
use edr_block_storage::{InsertBlockError, SparseBlockStorage};
use edr_chain_spec::ExecutableTransaction;
use edr_eth::{filter::OneOrMore, BlockSpec, PreEip1898BlockSpec};
use edr_primitives::{Address, HashSet, B256, U256};
use edr_receipt::{log::FilterLog, ReceiptTrait};
use edr_rpc_eth::{
    client::{EthRpcClient, RpcClientError},
    RpcBlockChainSpec,
};
use edr_rpc_spec::{RpcEthBlock, RpcTransaction};
use serde::{de::DeserializeOwned, Serialize};
use tokio::runtime;

#[derive(Debug)]
pub struct RemoteBlockchain<
    BlockReceiptT: ReceiptTrait,
    BlockT: Block<SignedTransactionT> + Clone,
    RpcBlockT: RpcBlockChainSpec,
    RpcReceiptT: DeserializeOwned + Serialize,
    RpcTransactionT: DeserializeOwned + Serialize,
    SignedTransactionT: ExecutableTransaction,
    const FORCE_CACHING: bool,
> {
    client: Arc<EthRpcClient<RpcBlockT, RpcReceiptT, RpcTransactionT>>,
    cache: RwLock<SparseBlockStorage<Arc<BlockReceiptT>, BlockT, SignedTransactionT>>,
    runtime: runtime::Handle,
}

impl<
        BlockReceiptT: ReceiptTrait,
        BlockT: Block<SignedTransactionT> + Clone,
        RpcBlockT: RpcBlockChainSpec,
        RpcReceiptT: DeserializeOwned + Serialize,
        RpcTransactionT: DeserializeOwned + Serialize,
        SignedTransactionT: ExecutableTransaction,
        const FORCE_CACHING: bool,
    >
    RemoteBlockchain<
        BlockReceiptT,
        BlockT,
        RpcBlockT,
        RpcReceiptT,
        RpcTransactionT,
        SignedTransactionT,
        FORCE_CACHING,
    >
{
    /// Constructs a new instance with the provided RPC client.
    pub fn new(
        client: Arc<EthRpcClient<RpcBlockT, RpcReceiptT, RpcTransactionT>>,
        runtime: runtime::Handle,
    ) -> Self {
        Self {
            client,
            cache: RwLock::new(SparseBlockStorage::default()),
            runtime,
        }
    }

    /// Retrieves the instance's RPC client.
    pub fn client(&self) -> &Arc<EthRpcClient<RpcBlockT, RpcReceiptT, RpcTransactionT>> {
        &self.client
    }

    pub async fn logs(
        &self,
        from_block: BlockSpec,
        to_block: BlockSpec,
        addresses: &HashSet<Address>,
        normalized_topics: &[Option<Vec<B256>>],
    ) -> Result<Vec<FilterLog>, RpcClientError> {
        self.client
            .get_logs_by_range(
                from_block,
                to_block,
                if addresses.len() > 1 {
                    Some(OneOrMore::Many(addresses.iter().copied().collect()))
                } else {
                    addresses
                        .iter()
                        .next()
                        .map(|address| OneOrMore::One(*address))
                },
                if normalized_topics.is_empty() {
                    None
                } else {
                    Some(
                        normalized_topics
                            .iter()
                            .map(|topics| {
                                topics.as_ref().and_then(|topics| {
                                    if topics.len() > 1 {
                                        Some(OneOrMore::Many(topics.clone()))
                                    } else {
                                        topics.first().map(|topic| OneOrMore::One(*topic))
                                    }
                                })
                            })
                            .collect(),
                    )
                },
            )
            .await
    }

    /// Retrieves the blockchain's runtime.
    pub fn runtime(&self) -> &runtime::Handle {
        &self.runtime
    }
}

/// An error that occurs when fetching a remote receipt.
#[derive(Debug, thiserror::Error)]
pub enum FetchRemoteReceiptError<RpcReceiptConversionErrorT> {
    /// Error converting a receipt
    #[error(transparent)]
    Conversion(RpcReceiptConversionErrorT),
    /// RPC client error
    #[error(transparent)]
    RpcClient(#[from] RpcClientError),
}

impl<
        BlockReceiptT: TryFrom<RpcReceiptT, Error = RpcReceiptConversionErrorT> + ReceiptTrait,
        BlockT: Block<SignedTransactionT> + Clone,
        RpcBlockT: RpcBlockChainSpec,
        RpcReceiptConversionErrorT,
        RpcReceiptT: DeserializeOwned + Serialize,
        RpcTransactionT: Default + DeserializeOwned + Serialize,
        SignedTransactionT: ExecutableTransaction,
        const FORCE_CACHING: bool,
    >
    RemoteBlockchain<
        BlockReceiptT,
        BlockT,
        RpcBlockT,
        RpcReceiptT,
        RpcTransactionT,
        SignedTransactionT,
        FORCE_CACHING,
    >
{
    /// Retrieves the receipt of the transaction with the provided hash, if it
    /// exists.
    #[cfg_attr(feature = "tracing", tracing::instrument(skip_all))]
    pub async fn receipt_by_transaction_hash(
        &self,
        transaction_hash: &B256,
    ) -> Result<Option<Arc<BlockReceiptT>>, FetchRemoteReceiptError<RpcReceiptConversionErrorT>>
    {
        let cache = self.cache.upgradable_read().await;

        if let Some(receipt) = cache.receipt_by_transaction_hash(transaction_hash) {
            Ok(Some(receipt.clone()))
        } else if let Some(receipt) = self
            .client
            .get_transaction_receipt(*transaction_hash)
            .await?
        {
            let receipt = receipt
                .try_into()
                .map_err(FetchRemoteReceiptError::Conversion)?;

            Ok(Some({
                let mut cache = RwLockUpgradableReadGuard::upgrade(cache).await;
                cache
                    .insert_receipt(Arc::new(receipt))
                    .expect("Already checked that receipt is not in cache")
                    .clone()
            }))
        } else {
            Ok(None)
        }
    }
}

/// An error that occurs when fetching a remote block.
#[derive(Debug, thiserror::Error)]
pub enum FetchRemoteBlockError<RpcBlockConversionErrorT> {
    /// Error converting a block
    #[error(transparent)]
    Conversion(RpcBlockConversionErrorT),
    /// RPC client error
    #[error(transparent)]
    RpcClient(#[from] RpcClientError),
}

/// An error that occurs when fetching and caching a remote block.
#[derive(Debug, thiserror::Error)]
enum FetchAndCacheRemoteBlockError<RpcBlockConversionErrorT> {
    /// Error converting a block
    #[error(transparent)]
    Conversion(RpcBlockConversionErrorT),
    /// Error when inserting a block into storage
    #[error(transparent)]
    Insert(#[from] InsertBlockError),
    /// RPC client error
    #[error(transparent)]
    RpcClient(#[from] RpcClientError),
}

impl<RpcBlockConversionErrorT> FetchAndCacheRemoteBlockError<RpcBlockConversionErrorT> {
    /// Converts the instance into a `FetchRemoteBlockError`.
    ///
    /// # Panics
    ///
    /// Panics if the value is an `InsertBlockError`.
    pub fn expect_no_insert_error(
        self,
        msg: &str,
    ) -> FetchRemoteBlockError<RpcBlockConversionErrorT> {
        match self {
            Self::Conversion(error) => FetchRemoteBlockError::Conversion(error),
            Self::Insert(error) => panic!("{msg}: {error}"),
            Self::RpcClient(error) => FetchRemoteBlockError::RpcClient(error),
        }
    }
}

impl<
        BlockReceiptT: Debug + ReceiptTrait,
        BlockT: Block<SignedTransactionT>
            + Clone
            + From<
                RemoteBlock<
                    BlockReceiptT,
                    RpcBlockT,
                    RpcReceiptT,
                    RpcTransactionT,
                    SignedTransactionT,
                >,
            >,
        RpcBlockConversionErrorT,
        RpcBlockT: RpcBlockChainSpec<
            RpcBlock<RpcTransactionT>: RpcEthBlock
                                           + TryInto<
                EthBlockData<SignedTransactionT>,
                Error = RpcBlockConversionErrorT,
            >,
        >,
        RpcReceiptT: serde::de::DeserializeOwned + serde::Serialize,
        RpcTransactionT: Default + serde::de::DeserializeOwned + serde::Serialize,
        SignedTransactionT: Debug + ExecutableTransaction,
        const FORCE_CACHING: bool,
    >
    RemoteBlockchain<
        BlockReceiptT,
        BlockT,
        RpcBlockT,
        RpcReceiptT,
        RpcTransactionT,
        SignedTransactionT,
        FORCE_CACHING,
    >
{
    /// Retrieves the block with the provided hash, if it exists.
    #[cfg_attr(feature = "tracing", tracing::instrument(skip_all))]
    pub async fn block_by_hash(
        &self,
        hash: &B256,
    ) -> Result<Option<BlockT>, FetchRemoteBlockError<RpcBlockConversionErrorT>> {
        let cache = self.cache.upgradable_read().await;

        if let Some(block) = cache.block_by_hash(hash).cloned() {
            return Ok(Some(block));
        }

        if let Some(block) = self
            .client
            .get_block_by_hash_with_transaction_data(*hash)
            .await?
        {
            self.fetch_and_cache_block(cache, block)
                .await
                .map_err(|error| {
                    error.expect_no_insert_error(
                        "Cache should not contain duplicate block nor transaction",
                    )
                })
                .map(Option::Some)
        } else {
            Ok(None)
        }
    }

    /// Retrieves the block with the provided number, if it exists.
    #[cfg_attr(feature = "tracing", tracing::instrument(skip_all))]
    pub async fn block_by_number(
        &self,
        number: u64,
    ) -> Result<BlockT, FetchRemoteBlockError<RpcBlockConversionErrorT>> {
        let cache = self.cache.upgradable_read().await;

        if let Some(block) = cache.block_by_number(number).cloned() {
            Ok(block)
        } else {
            let block = self
                .client
                .get_block_by_number_with_transaction_data(PreEip1898BlockSpec::Number(number))
                .await?;

            self.fetch_and_cache_block(cache, block)
                .await
                .map_err(|error| {
                    error.expect_no_insert_error(
                        "Cache should not contain duplicate block nor transaction",
                    )
                })
        }
    }

    /// Retrieves the total difficulty at the block with the provided hash.
    #[cfg_attr(feature = "tracing", tracing::instrument(skip_all))]
    pub async fn total_difficulty_by_hash(
        &self,
        hash: &B256,
    ) -> Result<Option<U256>, FetchRemoteBlockError<RpcBlockConversionErrorT>> {
        let cache = self.cache.upgradable_read().await;

        if let Some(difficulty) = cache.total_difficulty_by_hash(hash).cloned() {
            Ok(Some(difficulty))
        } else if let Some(block) = self
            .client
            .get_block_by_hash_with_transaction_data(*hash)
            .await?
        {
            // Geth has recently removed the total difficulty field from block RPC
            // responses, so we fall back to the terminal total difficulty of main net to
            // provide backwards compatibility.
            // TODO https://github.com/NomicFoundation/edr/issues/696
            let total_difficulty = *block
                .total_difficulty()
                .unwrap_or(&edr_defaults::TERMINAL_TOTAL_DIFFICULTY);

            self.fetch_and_cache_block(cache, block)
                .await
                .map_err(|error| {
                    error.expect_no_insert_error(
                        "Cache should not contain duplicate block nor transaction",
                    )
                })?;

            Ok(Some(total_difficulty))
        } else {
            Ok(None)
        }
    }

    /// Fetches detailed block information and caches the block.
    #[cfg_attr(feature = "tracing", tracing::instrument(skip_all))]
    async fn fetch_and_cache_block(
        &self,
        cache: RwLockUpgradableReadGuard<
            '_,
            SparseBlockStorage<Arc<BlockReceiptT>, BlockT, SignedTransactionT>,
        >,
        block: RpcBlockT::RpcBlock<RpcTransactionT>,
    ) -> Result<BlockT, FetchAndCacheRemoteBlockError<RpcBlockConversionErrorT>> {
        // Geth has recently removed the total difficulty field from block RPC
        // responses, so we fall back to the terminal total difficulty of main net to
        // provide backwards compatibility.
        // TODO https://github.com/NomicFoundation/edr/issues/696
        let total_difficulty = *block
            .total_difficulty()
            .unwrap_or(&edr_defaults::TERMINAL_TOTAL_DIFFICULTY);

        let block = RemoteBlock::new(block, self.client.clone(), self.runtime.clone())
            .map_err(FetchAndCacheRemoteBlockError::Conversion)?;

        let is_cacheable = FORCE_CACHING
            || self
                .client
                .is_cacheable_block_number(block.block_header().number)
                .await?;

        let block = BlockT::from(block);

        if is_cacheable {
            let mut remote_cache = RwLockUpgradableReadGuard::upgrade(cache).await;

            Ok(remote_cache.insert_block(block, total_difficulty)?.clone())
        } else {
            Ok(block)
        }
    }
}

impl<
        BlockReceiptT: Debug + ReceiptTrait,
        BlockT: Block<SignedTransactionT>
            + Clone
            + From<
                RemoteBlock<
                    BlockReceiptT,
                    RpcBlockT,
                    RpcReceiptT,
                    RpcTransactionT,
                    SignedTransactionT,
                >,
            >,
        RpcBlockConversionErrorT,
        RpcBlockT: RpcBlockChainSpec<
            RpcBlock<RpcTransactionT>: RpcEthBlock
                                           + TryInto<
                EthBlockData<SignedTransactionT>,
                Error = RpcBlockConversionErrorT,
            >,
        >,
        RpcReceiptT: serde::de::DeserializeOwned + serde::Serialize,
        RpcTransactionT: Default + RpcTransaction + serde::de::DeserializeOwned + serde::Serialize,
        SignedTransactionT: Debug + ExecutableTransaction,
        const FORCE_CACHING: bool,
    >
    RemoteBlockchain<
        BlockReceiptT,
        BlockT,
        RpcBlockT,
        RpcReceiptT,
        RpcTransactionT,
        SignedTransactionT,
        FORCE_CACHING,
    >
{
    /// Retrieves the block that contains a transaction with the provided hash,
    /// if it exists.
    #[cfg_attr(feature = "tracing", tracing::instrument(skip_all))]
    pub async fn block_by_transaction_hash(
        &self,
        transaction_hash: &B256,
    ) -> Result<Option<BlockT>, FetchRemoteBlockError<RpcBlockConversionErrorT>> {
        // This block ensure that the read lock is dropped
        {
            if let Some(block) = self
                .cache
                .read()
                .await
                .block_by_transaction_hash(transaction_hash)
                .cloned()
            {
                return Ok(Some(block));
            }
        }

        if let Some(transaction) = self
            .client
            .get_transaction_by_hash(*transaction_hash)
            .await?
        {
            self.block_by_hash(transaction.block_hash().expect("Not a pending transaction"))
                .await
        } else {
            Ok(None)
        }
    }
}

#[cfg(all(test, feature = "test-remote"))]
mod tests {
    use edr_chain_l1::L1ChainSpec;
    use edr_rpc_spec::EthRpcClientForChainSpec;
    use edr_runtime_spec::RemoteBlockForChainSpec;
    use edr_test_utils::env::get_alchemy_url;

    use super::*;

    #[tokio::test]
    async fn no_cache_for_unsafe_block_number() {
        let tempdir = tempfile::tempdir().expect("can create tempdir");

        let rpc_client = EthRpcClientForChainSpec::<L1ChainSpec>::new(
            &get_alchemy_url(),
            tempdir.path().to_path_buf(),
            None,
        )
        .expect("url ok");

        // Latest block number is always unsafe to cache
        let block_number = rpc_client.block_number().await.unwrap();

        let remote =
            RemoteBlockchain::<RemoteBlockForChainSpec<L1ChainSpec>, L1ChainSpec, false>::new(
                Arc::new(rpc_client),
                runtime::Handle::current(),
            );

        let _ = remote.block_by_number(block_number).await.unwrap();
        assert!(remote
            .cache
            .read()
            .await
            .block_by_number(block_number)
            .is_none());
    }
}
