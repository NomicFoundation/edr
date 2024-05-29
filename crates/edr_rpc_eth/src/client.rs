use std::fmt::Debug;

use async_trait::async_trait;
use edr_eth::{
    filter::OneOrMore, log::FilterLog, receipt::BlockReceipt, reward_percentile::RewardPercentile,
    AccountInfo, Address, BlockSpec, Bytes, PreEip1898BlockSpec, B256, U256,
};
use edr_rpc_client::{RpcClient, RpcClientError};

use crate::{fork::ForkMetadata, request_methods::RequestMethod, Transaction};

// Constrain parallel requests to avoid rate limiting on transport level and
// thundering herd during backoff.
const MAX_PARALLEL_REQUESTS: usize = 20;

#[async_trait]
pub trait EthClientExt {
    /// Calls `eth_feeHistory` and returns the fee history.
    async fn fee_history(
        &self,
        block_count: u64,
        newest_block: BlockSpec,
        reward_percentiles: Option<Vec<RewardPercentile>>,
    ) -> Result<FeeHistoryResult, RpcClientError>;

    /// Fetches the latest block number, chain ID, and network ID concurrently.
    async fn fork_metadata(&self) -> Result<ForkMetadata, RpcClientError>;

    /// Submits three concurrent RPC method invocations in order to obtain
    /// the set of data contained in [`AccountInfo`].
    async fn get_account_info(
        &self,
        address: &Address,
        block: Option<BlockSpec>,
    ) -> Result<AccountInfo, RpcClientError>;

    /// Fetches account infos for multiple addresses using concurrent requests.
    async fn get_account_infos(
        &self,
        addresses: &[Address],
        block: Option<BlockSpec>,
    ) -> Result<Vec<AccountInfo>, RpcClientError>;

    /// Calls `eth_getBlockByHash` and returns the transaction's hash.
    async fn get_block_by_hash(
        &self,
        hash: &B256,
    ) -> Result<Option<eth::Block<B256>>, RpcClientError>;

    /// Calls `eth_getBalance`.
    async fn get_balance(
        &self,
        address: &Address,
        block: Option<BlockSpec>,
    ) -> Result<U256, RpcClientError>;

    /// Calls `eth_getBlockByHash` and returns the transaction's data.
    async fn get_block_by_hash_with_transaction_data(
        &self,
        hash: &B256,
    ) -> Result<Option<eth::Block<eth::Transaction>>, RpcClientError>;

    /// Calls `eth_getBlockByNumber` and returns the transaction's hash.
    async fn get_block_by_number(
        &self,
        spec: PreEip1898BlockSpec,
    ) -> Result<Option<eth::Block<B256>>, RpcClientError>;

    /// Calls `eth_getBlockByNumber` and returns the transaction's data.
    async fn get_block_by_number_with_transaction_data(
        &self,
        spec: PreEip1898BlockSpec,
    ) -> Result<eth::Block<eth::Transaction>, RpcClientError>;

    /// Calls `eth_getCode`.
    async fn get_code(
        &self,
        address: &Address,
        block: Option<BlockSpec>,
    ) -> Result<Bytes, RpcClientError>;

    /// Calls `eth_getLogs` using a starting and ending block (inclusive).
    async fn get_logs_by_range(
        &self,
        from_block: BlockSpec,
        to_block: BlockSpec,
        address: Option<OneOrMore<Address>>,
        topics: Option<Vec<Option<OneOrMore<B256>>>>,
    ) -> Result<Vec<FilterLog>, RpcClientError>;

    /// Calls `eth_getTransactionByHash`.
    async fn get_transaction_by_hash(
        &self,
        tx_hash: &B256,
    ) -> Result<Option<eth::Transaction>, RpcClientError>;

    /// Calls `eth_getTransactionCount`.
    async fn get_transaction_count(
        &self,
        address: &Address,
        block: Option<BlockSpec>,
    ) -> Result<U256, RpcClientError>;

    /// Calls `eth_getTransactionReceipt`.
    async fn get_transaction_receipt(
        &self,
        tx_hash: &B256,
    ) -> Result<Option<BlockReceipt>, RpcClientError>;

    /// Methods for retrieving multiple transaction receipts using concurrent
    /// requests.
    async fn get_transaction_receipts(
        &self,
        hashes: impl IntoIterator<Item = &B256> + Debug,
    ) -> Result<Option<Vec<BlockReceipt>>, RpcClientError>;

    /// Calls `eth_getStorageAt`.
    async fn get_storage_at(
        &self,
        address: &Address,
        position: U256,
        block: Option<BlockSpec>,
    ) -> Result<Option<U256>, RpcClientError>;

    /// Calls `net_version`.
    async fn network_id(&self) -> Result<u64, RpcClientError>;
}

impl EthClientExt for RpcClient<RequestMethod> {
    #[cfg_attr(feature = "tracing", tracing::instrument(level = "trace", skip(self)))]
    async fn fee_history(
        &self,
        block_count: u64,
        newest_block: BlockSpec,
        reward_percentiles: Option<Vec<RewardPercentile>>,
    ) -> Result<FeeHistoryResult, RpcClientError> {
        self.call(RequestMethod::FeeHistory(
            U256::from(block_count),
            newest_block,
            reward_percentiles,
        ))
        .await
    }

    #[cfg_attr(feature = "tracing", tracing::instrument(level = "trace", skip(self)))]
    async fn fork_metadata(&self) -> Result<ForkMetadata, RpcClientError> {
        let network_id = self.network_id();
        let block_number = self.block_number();
        let chain_id = self.chain_id();

        let (network_id, block_number, chain_id) =
            tokio::try_join!(network_id, block_number, chain_id)?;

        Ok(ForkMetadata {
            chain_id,
            network_id,
            latest_block_number: block_number,
        })
    }

    #[cfg_attr(feature = "tracing", tracing::instrument(level = "trace", skip(self)))]
    async fn get_account_info(
        &self,
        address: &Address,
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

    #[cfg_attr(feature = "tracing", tracing::instrument(level = "trace", skip(self)))]
    async fn get_account_infos(
        &self,
        addresses: &[Address],
        block: Option<BlockSpec>,
    ) -> Result<Vec<AccountInfo>, RpcClientError> {
        futures::stream::iter(addresses.iter())
            .map(|address| self.get_account_info(address, block.clone()))
            .buffered(MAX_PARALLEL_REQUESTS / 3 + 1)
            .collect::<Vec<Result<AccountInfo, RpcClientError>>>()
            .await
            .into_iter()
            .collect()
    }

    #[cfg_attr(feature = "tracing", tracing::instrument(level = "trace", skip(self)))]
    async fn get_block_by_hash(
        &self,
        hash: &B256,
    ) -> Result<Option<eth::Block<B256>>, RpcClientError> {
        self.call(MethodT::GetBlockByHash(*hash, false)).await
    }

    #[cfg_attr(feature = "tracing", tracing::instrument(level = "trace", skip(self)))]
    async fn get_balance(
        &self,
        address: &Address,
        block: Option<BlockSpec>,
    ) -> Result<U256, RpcClientError> {
        self.call(MethodT::GetBalance(*address, block)).await
    }

    #[cfg_attr(feature = "tracing", tracing::instrument(level = "trace", skip(self)))]
    async fn get_block_by_hash_with_transaction_data(
        &self,
        hash: &B256,
    ) -> Result<Option<eth::Block<eth::Transaction>>, RpcClientError> {
        self.call(MethodT::GetBlockByHash(*hash, true)).await
    }

    #[cfg_attr(feature = "tracing", tracing::instrument(level = "trace", skip(self)))]
    async fn get_block_by_number(
        &self,
        spec: PreEip1898BlockSpec,
    ) -> Result<Option<eth::Block<B256>>, RpcClientError> {
        self.call_with_resolver(
            MethodT::GetBlockByNumber(spec, false),
            |block: &Option<eth::Block<B256>>| block.as_ref().and_then(|block| block.number),
        )
        .await
    }

    #[cfg_attr(feature = "tracing", tracing::instrument(level = "trace", skip(self)))]
    async fn get_block_by_number_with_transaction_data(
        &self,
        spec: PreEip1898BlockSpec,
    ) -> Result<eth::Block<eth::Transaction>, RpcClientError> {
        self.call_with_resolver(
            MethodT::GetBlockByNumber(spec, true),
            |block: &eth::Block<eth::Transaction>| block.number,
        )
        .await
    }

    #[cfg_attr(feature = "tracing", tracing::instrument(level = "trace", skip(self)))]
    async fn get_code(
        &self,
        address: &Address,
        block: Option<BlockSpec>,
    ) -> Result<Bytes, RpcClientError> {
        self.call(MethodT::GetCode(*address, block)).await
    }

    #[cfg_attr(feature = "tracing", tracing::instrument(level = "trace", skip(self)))]
    async fn get_logs_by_range(
        &self,
        from_block: BlockSpec,
        to_block: BlockSpec,
        address: Option<OneOrMore<Address>>,
        topics: Option<Vec<Option<OneOrMore<B256>>>>,
    ) -> Result<Vec<FilterLog>, RpcClientError> {
        self.call(MethodT::GetLogs(LogFilterOptions {
            from_block: Some(from_block),
            to_block: Some(to_block),
            block_hash: None,
            address,
            topics,
        }))
        .await
    }

    #[cfg_attr(feature = "tracing", tracing::instrument(level = "trace", skip(self)))]
    async fn get_transaction_by_hash(
        &self,
        tx_hash: &B256,
    ) -> Result<Option<Transaction>, RpcClientError> {
        self.call(MethodT::GetTransactionByHash(*tx_hash)).await
    }

    #[cfg_attr(feature = "tracing", tracing::instrument(level = "trace", skip(self)))]
    async fn get_transaction_count(
        &self,
        address: &Address,
        block: Option<BlockSpec>,
    ) -> Result<U256, RpcClientError> {
        self.call(MethodT::GetTransactionCount(*address, block))
            .await
    }

    #[cfg_attr(feature = "tracing", tracing::instrument(level = "trace", skip(self)))]
    async fn get_transaction_receipt(
        &self,
        tx_hash: &B256,
    ) -> Result<Option<BlockReceipt>, RpcClientError> {
        self.call(MethodT::GetTransactionReceipt(*tx_hash)).await
    }

    #[cfg_attr(feature = "tracing", tracing::instrument(level = "trace", skip(self)))]
    async fn get_transaction_receipts(
        &self,
        hashes: impl IntoIterator<Item = &B256> + Debug,
    ) -> Result<Option<Vec<BlockReceipt>>, RpcClientError> {
        let requests = hashes
            .into_iter()
            .map(|transaction_hash| self.get_transaction_receipt(transaction_hash));

        futures::stream::iter(requests)
            .buffered(MAX_PARALLEL_REQUESTS)
            .collect::<Vec<Result<Option<BlockReceipt>, RpcClientError>>>()
            .await
            .into_iter()
            .collect()
    }

    #[cfg_attr(feature = "tracing", tracing::instrument(level = "trace", skip(self)))]
    async fn get_storage_at(
        &self,
        address: &Address,
        position: U256,
        block: Option<BlockSpec>,
    ) -> Result<Option<U256>, RpcClientError> {
        self.call(MethodT::GetStorageAt(*address, position, block))
            .await
    }

    #[cfg_attr(feature = "tracing", tracing::instrument(level = "trace", skip(self)))]
    async fn network_id(&self) -> Result<u64, RpcClientError> {
        self.call::<U64>(MethodT::NetVersion(()))
            .await
            .map(|network_id| network_id.as_limbs()[0])
    }
}
