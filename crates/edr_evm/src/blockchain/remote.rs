use std::sync::Arc;

use async_rwlock::{RwLock, RwLockUpgradableReadGuard};
use edr_eth::{
    filter::OneOrMore, log::FilterLog, receipt::BlockReceipt, Address, BlockSpec,
    PreEip1898BlockSpec, B256, U256,
};
use edr_rpc_eth::client::EthRpcClient;
use revm::primitives::HashSet;
use tokio::runtime;

use super::storage::SparseBlockchainStorage;
use crate::{
    blockchain::ForkedBlockchainError, chain_spec::ChainSpec,
    transaction::remote::EthRpcTransaction as _, Block, EthRpcBlock as _, RemoteBlock,
};

#[derive(Debug)]
pub struct RemoteBlockchain<BlockT, ChainSpecT, const FORCE_CACHING: bool>
where
    BlockT: Block<ChainSpecT> + Clone,
    ChainSpecT: ChainSpec,
{
    client: Arc<EthRpcClient<ChainSpecT>>,
    cache: RwLock<SparseBlockchainStorage<BlockT, ChainSpecT>>,
    runtime: runtime::Handle,
}

impl<BlockT, ChainSpecT, const FORCE_CACHING: bool>
    RemoteBlockchain<BlockT, ChainSpecT, FORCE_CACHING>
where
    BlockT: Block<ChainSpecT> + Clone + From<RemoteBlock<ChainSpecT>>,
    ChainSpecT: ChainSpec,
{
    /// Constructs a new instance with the provided RPC client.
    pub fn new(client: Arc<EthRpcClient<ChainSpecT>>, runtime: runtime::Handle) -> Self {
        Self {
            client,
            cache: RwLock::new(SparseBlockchainStorage::default()),
            runtime,
        }
    }

    /// Retrieves the block with the provided hash, if it exists.
    #[cfg_attr(feature = "tracing", tracing::instrument(skip_all))]
    pub async fn block_by_hash(
        &self,
        hash: &B256,
    ) -> Result<Option<BlockT>, ForkedBlockchainError<ChainSpecT>> {
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
    ) -> Result<BlockT, ForkedBlockchainError<ChainSpecT>> {
        let cache = self.cache.upgradable_read().await;

        if let Some(block) = cache.block_by_number(number).cloned() {
            Ok(block)
        } else {
            let block = self
                .client
                .get_block_by_number_with_transaction_data(PreEip1898BlockSpec::Number(number))
                .await?;

            self.fetch_and_cache_block(cache, block).await
        }
    }

    /// Retrieves the block that contains a transaction with the provided hash,
    /// if it exists.
    #[cfg_attr(feature = "tracing", tracing::instrument(skip_all))]
    pub async fn block_by_transaction_hash(
        &self,
        transaction_hash: &B256,
    ) -> Result<Option<BlockT>, ForkedBlockchainError<ChainSpecT>> {
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

    /// Retrieves the instance's RPC client.
    pub fn client(&self) -> &Arc<EthRpcClient<ChainSpecT>> {
        &self.client
    }

    pub async fn logs(
        &self,
        from_block: BlockSpec,
        to_block: BlockSpec,
        addresses: &HashSet<Address>,
        normalized_topics: &[Option<Vec<B256>>],
    ) -> Result<Vec<FilterLog>, ForkedBlockchainError<ChainSpecT>> {
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
            .map_err(ForkedBlockchainError::RpcClient)
    }

    /// Retrieves the receipt of the transaction with the provided hash, if it
    /// exists.
    #[cfg_attr(feature = "tracing", tracing::instrument(skip_all))]
    pub async fn receipt_by_transaction_hash(
        &self,
        transaction_hash: &B256,
    ) -> Result<Option<Arc<BlockReceipt>>, ForkedBlockchainError<ChainSpecT>> {
        let cache = self.cache.upgradable_read().await;

        if let Some(receipt) = cache.receipt_by_transaction_hash(transaction_hash) {
            Ok(Some(receipt.clone()))
        } else if let Some(receipt) = self
            .client
            .get_transaction_receipt(*transaction_hash)
            .await?
        {
            Ok(Some({
                let mut cache = RwLockUpgradableReadGuard::upgrade(cache).await;
                cache.insert_receipt(receipt)?.clone()
            }))
        } else {
            Ok(None)
        }
    }

    /// Retrieves the blockchain's runtime.
    pub fn runtime(&self) -> &runtime::Handle {
        &self.runtime
    }

    /// Retrieves the total difficulty at the block with the provided hash.
    #[cfg_attr(feature = "tracing", tracing::instrument(skip_all))]
    pub async fn total_difficulty_by_hash(
        &self,
        hash: &B256,
    ) -> Result<Option<U256>, ForkedBlockchainError<ChainSpecT>> {
        let cache = self.cache.upgradable_read().await;

        if let Some(difficulty) = cache.total_difficulty_by_hash(hash).cloned() {
            Ok(Some(difficulty))
        } else if let Some(block) = self
            .client
            .get_block_by_hash_with_transaction_data(*hash)
            .await?
        {
            let total_difficulty = *block
                .total_difficulty()
                .expect("Must be present as this is not a pending transaction");

            self.fetch_and_cache_block(cache, block).await?;

            Ok(Some(total_difficulty))
        } else {
            Ok(None)
        }
    }

    /// Fetches detailed block information and caches the block.
    #[cfg_attr(feature = "tracing", tracing::instrument(skip_all))]
    async fn fetch_and_cache_block(
        &self,
        cache: RwLockUpgradableReadGuard<'_, SparseBlockchainStorage<BlockT, ChainSpecT>>,
        block: ChainSpecT::RpcBlock<ChainSpecT::RpcTransaction>,
    ) -> Result<BlockT, ForkedBlockchainError<ChainSpecT>> {
        let total_difficulty = *block
            .total_difficulty()
            .expect("Must be present as this is not a pending block");

        let block = RemoteBlock::new(block, self.client.clone(), self.runtime.clone())
            .map_err(ForkedBlockchainError::BlockCreation)?;

        let is_cacheable = FORCE_CACHING
            || self
                .client
                .is_cacheable_block_number(block.header().number)
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

#[cfg(all(test, feature = "test-remote"))]
mod tests {
    use edr_test_utils::env::get_alchemy_url;

    use super::*;
    use crate::chain_spec::L1ChainSpec;

    #[tokio::test]
    async fn no_cache_for_unsafe_block_number() {
        let tempdir = tempfile::tempdir().expect("can create tempdir");

        let rpc_client = EthRpcClient::<L1ChainSpec>::new(
            &get_alchemy_url(),
            tempdir.path().to_path_buf(),
            None,
        )
        .expect("url ok");

        // Latest block number is always unsafe to cache
        let block_number = rpc_client.block_number().await.unwrap();

        let remote = RemoteBlockchain::<RemoteBlock<L1ChainSpec>, L1ChainSpec, false>::new(
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
