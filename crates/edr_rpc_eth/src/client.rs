use std::{fmt::Debug, path::PathBuf};

use derive_where::derive_where;
use edr_eth::{
    account::{AccountInfo, KECCAK_EMPTY},
    fee_history::FeeHistoryResult,
    filter::{LogFilterOptions, OneOrMore},
    log::FilterLog,
    reward_percentile::RewardPercentile,
    Address, BlockSpec, Bytecode, Bytes, PreEip1898BlockSpec, B256, U256, U64,
};
use edr_rpc_client::RpcClient;
pub use edr_rpc_client::{header, HeaderMap, RpcClientError};
use futures::StreamExt;

use crate::{
    fork::ForkMetadata,
    request_methods::RequestMethod,
    spec::{GetBlockNumber, RpcSpec},
};

// Constrain parallel requests to avoid rate limiting on transport level and
// thundering herd during backoff.
const MAX_PARALLEL_REQUESTS: usize = 20;

#[derive_where(Debug)]
pub struct EthRpcClient<RpcSpecT: RpcSpec> {
    inner: RpcClient<RequestMethod>,
    phantom: std::marker::PhantomData<RpcSpecT>,
}

impl<RpcSpecT: RpcSpec> EthRpcClient<RpcSpecT> {
    /// Creates a new instance, given a remote node URL.
    ///
    /// The cache directory is the global EDR cache directory configured by the
    /// user.
    pub fn new(
        url: &str,
        cache_dir: PathBuf,
        extra_headers: Option<HeaderMap>,
    ) -> Result<Self, RpcClientError> {
        let inner = RpcClient::new(url, cache_dir, extra_headers)?;
        Ok(Self {
            inner,
            phantom: std::marker::PhantomData,
        })
    }

    /// Calls `eth_blockNumber` and returns the block number.
    #[cfg_attr(feature = "tracing", tracing::instrument(level = "trace", skip(self)))]
    pub async fn block_number(&self) -> Result<u64, RpcClientError> {
        self.inner.block_number().await
    }

    /// Calls `eth_chainId` and returns the chain ID.
    #[cfg_attr(feature = "tracing", tracing::instrument(level = "trace", skip(self)))]
    pub async fn chain_id(&self) -> Result<u64, RpcClientError> {
        self.inner.chain_id().await
    }

    #[cfg_attr(feature = "tracing", tracing::instrument(level = "trace", skip(self)))]
    /// Calls `eth_feeHistory` and returns the fee history.
    pub async fn fee_history(
        &self,
        block_count: u64,
        newest_block: BlockSpec,
        reward_percentiles: Option<Vec<RewardPercentile>>,
    ) -> Result<FeeHistoryResult, RpcClientError> {
        self.inner
            .call(RequestMethod::FeeHistory(
                U256::from(block_count),
                newest_block,
                reward_percentiles,
            ))
            .await
    }

    #[cfg_attr(feature = "tracing", tracing::instrument(level = "trace", skip(self)))]
    /// Fetches the latest block number, chain ID, and network ID concurrently.
    pub async fn fork_metadata(&self) -> Result<ForkMetadata, RpcClientError> {
        let network_id = self.network_id();
        let block_number = self.inner.block_number();
        let chain_id = self.inner.chain_id();

        let (network_id, block_number, chain_id) =
            tokio::try_join!(network_id, block_number, chain_id)?;

        Ok(ForkMetadata {
            chain_id,
            network_id,
            latest_block_number: block_number,
        })
    }

    /// Submits three concurrent RPC method invocations in order to obtain
    /// the set of data contained in [`AccountInfo`].
    #[cfg_attr(feature = "tracing", tracing::instrument(level = "trace", skip(self)))]
    pub async fn get_account_info(
        &self,
        address: Address,
        block: Option<BlockSpec>,
    ) -> Result<AccountInfo, RpcClientError> {
        let balance = self.get_balance(address, block.clone());
        let nonce = self.get_transaction_count(address, block.clone());
        let code = self.get_code(address, block.clone());

        let (balance, nonce, code) = tokio::try_join!(balance, nonce, code)?;

        let code = if code.is_empty() {
            None
        } else {
            Some(Bytecode::new_raw(code))
        };

        Ok(AccountInfo {
            balance,
            code_hash: code.as_ref().map_or(KECCAK_EMPTY, Bytecode::hash_slow),
            code,
            nonce: nonce.to(),
        })
    }

    /// Fetches account infos for multiple addresses using concurrent requests.
    #[cfg_attr(feature = "tracing", tracing::instrument(level = "trace", skip(self)))]
    pub async fn get_account_infos(
        &self,
        addresses: &[Address],
        block: Option<BlockSpec>,
    ) -> Result<Vec<AccountInfo>, RpcClientError> {
        futures::stream::iter(addresses.iter())
            .map(|address| self.get_account_info(*address, block.clone()))
            .buffered(MAX_PARALLEL_REQUESTS / 3 + 1)
            .collect::<Vec<Result<AccountInfo, RpcClientError>>>()
            .await
            .into_iter()
            .collect()
    }

    /// Calls `eth_getBlockByHash` and returns the transaction's hash.
    #[cfg_attr(feature = "tracing", tracing::instrument(level = "trace", skip(self)))]
    pub async fn get_block_by_hash(
        &self,
        hash: B256,
    ) -> Result<Option<RpcSpecT::RpcBlock<B256>>, RpcClientError> {
        self.inner
            .call(RequestMethod::GetBlockByHash(hash, false))
            .await
    }

    /// Calls `eth_getBalance`.
    #[cfg_attr(feature = "tracing", tracing::instrument(level = "trace", skip(self)))]
    pub async fn get_balance(
        &self,
        address: Address,
        block: Option<BlockSpec>,
    ) -> Result<U256, RpcClientError> {
        self.inner
            .call(RequestMethod::GetBalance(address, block))
            .await
    }

    /// Calls `eth_getBlockByHash` and returns the transaction's data.
    #[cfg_attr(feature = "tracing", tracing::instrument(level = "trace", skip(self)))]
    pub async fn get_block_by_hash_with_transaction_data(
        &self,
        hash: B256,
    ) -> Result<Option<RpcSpecT::RpcBlock<RpcSpecT::RpcTransaction>>, RpcClientError> {
        self.inner
            .call(RequestMethod::GetBlockByHash(hash, true))
            .await
    }

    /// Calls `eth_getBlockByNumber` and returns the transaction's hash.
    #[cfg_attr(feature = "tracing", tracing::instrument(level = "trace", skip(self)))]
    pub async fn get_block_by_number(
        &self,
        spec: PreEip1898BlockSpec,
    ) -> Result<Option<RpcSpecT::RpcBlock<B256>>, RpcClientError> {
        self.inner
            .call_with_resolver(
                RequestMethod::GetBlockByNumber(spec, false),
                |block: &Option<RpcSpecT::RpcBlock<B256>>| {
                    block.as_ref().and_then(GetBlockNumber::number)
                },
            )
            .await
    }

    /// Calls `eth_getBlockByNumber` and returns the transaction's data.
    #[cfg_attr(feature = "tracing", tracing::instrument(level = "trace", skip(self)))]
    pub async fn get_block_by_number_with_transaction_data(
        &self,
        spec: PreEip1898BlockSpec,
    ) -> Result<RpcSpecT::RpcBlock<RpcSpecT::RpcTransaction>, RpcClientError> {
        self.inner
            .call_with_resolver(
                RequestMethod::GetBlockByNumber(spec, true),
                |block: &RpcSpecT::RpcBlock<RpcSpecT::RpcTransaction>| block.number(),
            )
            .await
    }

    /// Calls `eth_getCode`.
    #[cfg_attr(feature = "tracing", tracing::instrument(level = "trace", skip(self)))]
    pub async fn get_code(
        &self,
        address: Address,
        block: Option<BlockSpec>,
    ) -> Result<Bytes, RpcClientError> {
        self.inner
            .call(RequestMethod::GetCode(address, block))
            .await
    }

    /// Calls `eth_getLogs` using a starting and ending block (inclusive).
    #[cfg_attr(feature = "tracing", tracing::instrument(level = "trace", skip(self)))]
    pub async fn get_logs_by_range(
        &self,
        from_block: BlockSpec,
        to_block: BlockSpec,
        address: Option<OneOrMore<Address>>,
        topics: Option<Vec<Option<OneOrMore<B256>>>>,
    ) -> Result<Vec<FilterLog>, RpcClientError> {
        self.inner
            .call(RequestMethod::GetLogs(LogFilterOptions {
                from_block: Some(from_block),
                to_block: Some(to_block),
                block_hash: None,
                address,
                topics,
            }))
            .await
    }

    /// Calls `eth_getTransactionByHash`.
    #[cfg_attr(feature = "tracing", tracing::instrument(level = "trace", skip(self)))]
    pub async fn get_transaction_by_hash(
        &self,
        tx_hash: B256,
    ) -> Result<Option<RpcSpecT::RpcTransaction>, RpcClientError> {
        self.inner
            .call(RequestMethod::GetTransactionByHash(tx_hash))
            .await
    }

    /// Calls `eth_getTransactionCount`.
    #[cfg_attr(feature = "tracing", tracing::instrument(level = "trace", skip(self)))]
    pub async fn get_transaction_count(
        &self,
        address: Address,
        block: Option<BlockSpec>,
    ) -> Result<U256, RpcClientError> {
        self.inner
            .call(RequestMethod::GetTransactionCount(address, block))
            .await
    }

    /// Calls `eth_getTransactionReceipt`.
    #[cfg_attr(feature = "tracing", tracing::instrument(level = "trace", skip(self)))]
    pub async fn get_transaction_receipt(
        &self,
        tx_hash: B256,
    ) -> Result<Option<RpcSpecT::RpcReceipt>, RpcClientError> {
        self.inner
            .call(RequestMethod::GetTransactionReceipt(tx_hash))
            .await
    }

    /// Methods for retrieving multiple transaction receipts using concurrent
    /// requests.
    #[cfg_attr(feature = "tracing", tracing::instrument(level = "trace", skip(self)))]
    pub async fn get_transaction_receipts(
        &self,
        hashes: impl IntoIterator<Item = &B256> + Debug,
    ) -> Result<Option<Vec<RpcSpecT::RpcReceipt>>, RpcClientError> {
        let requests = hashes
            .into_iter()
            .map(|transaction_hash| self.get_transaction_receipt(*transaction_hash))
            .collect::<Vec<_>>();

        futures::stream::iter(requests)
            .buffered(MAX_PARALLEL_REQUESTS)
            .collect::<Vec<Result<Option<RpcSpecT::RpcReceipt>, RpcClientError>>>()
            .await
            .into_iter()
            .collect()
    }

    /// Calls `eth_getStorageAt`.
    #[cfg_attr(feature = "tracing", tracing::instrument(level = "trace", skip(self)))]
    pub async fn get_storage_at(
        &self,
        address: Address,
        position: U256,
        block: Option<BlockSpec>,
    ) -> Result<Option<U256>, RpcClientError> {
        self.inner
            .call(RequestMethod::GetStorageAt(address, position, block))
            .await
    }

    /// Whether the block number should be cached based on its depth.
    #[cfg_attr(feature = "tracing", tracing::instrument(level = "trace", skip(self)))]
    pub async fn is_cacheable_block_number(
        &self,
        block_number: u64,
    ) -> Result<bool, RpcClientError> {
        self.inner.is_cacheable_block_number(block_number).await
    }

    /// Calls `net_version`.
    #[cfg_attr(feature = "tracing", tracing::instrument(level = "trace", skip(self)))]
    pub async fn network_id(&self) -> Result<u64, RpcClientError> {
        self.inner
            .call::<U64>(RequestMethod::NetVersion(()))
            .await
            .map(|network_id| network_id.as_limbs()[0])
    }
}
