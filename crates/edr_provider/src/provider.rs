use std::sync::Arc;

use edr_eth::{
    l1,
    transaction::{IsEip155, IsEip4844, TransactionMut, TransactionType, TransactionValidation},
};
use edr_evm::blockchain::BlockchainErrorForChainSpec;
use edr_solidity::contract_decoder::ContractDecoder;
use parking_lot::Mutex;
use tokio::{runtime, sync::Mutex as AsyncMutex, task};

use crate::{
    data::ProviderData,
    error::{CreationErrorForChainSpec, ProviderError, ProviderErrorForChainSpec},
    interval::IntervalMiner,
    logger::SyncLogger,
    mock::SyncCallOverride,
    requests::{
        debug,
        eth::{self, handle_set_interval_mining},
        hardhat::{self, rpc_types::ResetProviderConfig},
        MethodInvocation, ProviderRequest,
    },
    spec::{ProviderSpec, SyncProviderSpec},
    time::{CurrentTime, TimeSinceEpoch},
    to_json, to_json_with_trace, to_json_with_traces, ProviderConfig, ResponseWithTraces,
    SyncSubscriberCallback, PRIVATE_RPC_METHODS,
};

/// A JSON-RPC provider for Ethereum.
///
/// Add a layer in front that handles this
///
/// ```rust,ignore
/// let RpcRequest {
///     version,
///     method: request,
///     id,
/// } = request;
///
/// if version != jsonrpc::Version::V2_0 {
///     return Err(ProviderError::RpcVersion(version));
/// }
///
/// fn to_response(
///     id: jsonrpc::Id,
///     result: Result<serde_json::Value, ProviderErrorForChainSpec<ChainSpecT>,
/// ) -> jsonrpc::Response<serde_json::Value> { let data = match result {
///   Ok(result) => jsonrpc::ResponseData::Success { result }, Err(error) =>
///   jsonrpc::ResponseData::Error { error: jsonrpc::Error { code: -32000,
///   message: error.to_string(), data: None, }, }, };
///
///     jsonrpc::Response {
///         jsonrpc: jsonrpc::Version::V2_0,
///         id,
///         data,
///     }
/// }
/// ```
pub struct Provider<ChainSpecT: ProviderSpec<TimerT>, TimerT: Clone + TimeSinceEpoch = CurrentTime>
{
    data: Arc<AsyncMutex<ProviderData<ChainSpecT, TimerT>>>,
    /// Interval miner runs in the background, if enabled. It holds the data
    /// mutex, so it needs to internally check for cancellation/self-destruction
    /// while async-awaiting the lock to avoid a deadlock.
    interval_miner: Arc<Mutex<Option<IntervalMiner<ChainSpecT, TimerT>>>>,
    runtime: runtime::Handle,
}

impl<ChainSpecT: SyncProviderSpec<TimerT>, TimerT: Clone + TimeSinceEpoch>
    Provider<ChainSpecT, TimerT>
{
    /// Blocking method to log a failed deserialization.
    pub fn log_failed_deserialization(
        &self,
        method_name: &str,
        error: &ProviderErrorForChainSpec<ChainSpecT>,
    ) -> Result<(), ProviderErrorForChainSpec<ChainSpecT>> {
        let mut data = task::block_in_place(|| self.runtime.block_on(self.data.lock()));
        data.logger_mut()
            .print_method_logs(method_name, Some(error))
            .map_err(ProviderError::Logger)
    }
}

impl<
        ChainSpecT: SyncProviderSpec<
            TimerT,
            BlockEnv: Default,
            SignedTransaction: Default
                                   + TransactionValidation<
                ValidationError: From<l1::InvalidTransaction> + PartialEq,
            >,
        >,
        TimerT: Clone + TimeSinceEpoch,
    > Provider<ChainSpecT, TimerT>
{
    /// Constructs a new instance.
    pub fn new(
        runtime: runtime::Handle,
        logger: Box<
            dyn SyncLogger<ChainSpecT, BlockchainError = BlockchainErrorForChainSpec<ChainSpecT>>,
        >,
        subscriber_callback: Box<dyn SyncSubscriberCallback<ChainSpecT>>,
        config: ProviderConfig<ChainSpecT::Hardfork>,
        contract_decoder: Arc<ContractDecoder>,
        timer: TimerT,
    ) -> Result<Self, CreationErrorForChainSpec<ChainSpecT>> {
        let data = ProviderData::new(
            runtime.clone(),
            logger,
            subscriber_callback,
            config.clone(),
            contract_decoder,
            timer,
        )?;
        let data = Arc::new(AsyncMutex::new(data));

        let interval_miner = config
            .mining
            .interval
            .as_ref()
            .map(|config| IntervalMiner::new(runtime.clone(), config.clone(), data.clone()));

        let interval_miner = Arc::new(Mutex::new(interval_miner));

        Ok(Self {
            data,
            interval_miner,
            runtime,
        })
    }

    /// Set to `true` to make the traces returned with `eth_call`,
    /// `eth_estimateGas`, `eth_sendRawTransaction`, `eth_sendTransaction`,
    /// `evm_mine`, `hardhat_mine` include the full stack and memory. Set to
    /// `false` to disable this.
    pub fn set_call_override_callback(
        &self,
        call_override_callback: Option<Arc<dyn SyncCallOverride>>,
    ) {
        let mut data = task::block_in_place(|| self.runtime.block_on(self.data.lock()));
        data.set_call_override_callback(call_override_callback);
    }

    pub fn set_verbose_tracing(&self, enabled: bool) {
        let mut data = task::block_in_place(|| self.runtime.block_on(self.data.lock()));
        data.set_verbose_tracing(enabled);
    }
}

impl<
        ChainSpecT: SyncProviderSpec<
            TimerT,
            BlockEnv: Clone + Default,
            PooledTransaction: IsEip155,
            SignedTransaction: Default
                                   + TransactionMut
                                   + TransactionType<Type: IsEip4844>
                                   + TransactionValidation<
                ValidationError: From<l1::InvalidTransaction> + PartialEq,
            >,
        >,
        TimerT: Clone + TimeSinceEpoch,
    > Provider<ChainSpecT, TimerT>
{
    /// Blocking method to handle a request.
    pub fn handle_request(
        &self,
        request: ProviderRequest<ChainSpecT>,
    ) -> Result<ResponseWithTraces<ChainSpecT::HaltReason>, ProviderErrorForChainSpec<ChainSpecT>>
    {
        let mut data = task::block_in_place(|| self.runtime.block_on(self.data.lock()));

        let response = match request {
            ProviderRequest::Single(request) => self.handle_single_request(&mut data, *request),
            ProviderRequest::Batch(requests) => self.handle_batch_request(&mut data, requests),
        }?;

        Ok(response)
    }

    /// Handles a batch of JSON requests for an execution provider.
    fn handle_batch_request(
        &self,
        data: &mut ProviderData<ChainSpecT, TimerT>,
        request: Vec<MethodInvocation<ChainSpecT>>,
    ) -> Result<ResponseWithTraces<ChainSpecT::HaltReason>, ProviderErrorForChainSpec<ChainSpecT>>
    {
        let mut results = Vec::new();
        let mut traces = Vec::new();

        for req in request {
            let response = self.handle_single_request(data, req)?;
            results.push(response.result);
            traces.extend(response.traces);
        }

        let result = serde_json::to_value(results).map_err(ProviderError::Serialization)?;
        Ok(ResponseWithTraces { result, traces })
    }

    fn handle_single_request(
        &self,
        data: &mut ProviderData<ChainSpecT, TimerT>,
        request: MethodInvocation<ChainSpecT>,
    ) -> Result<ResponseWithTraces<ChainSpecT::HaltReason>, ProviderErrorForChainSpec<ChainSpecT>>
    {
        let method_name = if data.logger_mut().is_enabled() {
            let method_name = request.method_name();
            if PRIVATE_RPC_METHODS.contains(method_name) {
                None
            } else {
                Some(method_name)
            }
        } else {
            None
        };

        let result = match request {
            // eth_* method
            MethodInvocation::Accounts(()) => {
                eth::handle_accounts_request(data).and_then(to_json::<_, ChainSpecT>)
            }
            MethodInvocation::BlobBaseFee(()) => {
                eth::handle_blob_base_fee(data).and_then(to_json::<_, ChainSpecT>)
            }
            MethodInvocation::BlockNumber(()) => {
                eth::handle_block_number_request(data).and_then(to_json::<_, ChainSpecT>)
            }
            MethodInvocation::Call(request, block_spec, state_overrides) => {
                eth::handle_call_request(data, request, block_spec, state_overrides)
                    .and_then(to_json_with_trace::<_, ChainSpecT>)
            }
            MethodInvocation::ChainId(()) => {
                eth::handle_chain_id_request(data).and_then(to_json::<_, ChainSpecT>)
            }
            MethodInvocation::Coinbase(()) => {
                eth::handle_coinbase_request(data).and_then(to_json::<_, ChainSpecT>)
            }
            MethodInvocation::EstimateGas(call_request, block_spec) => {
                eth::handle_estimate_gas(data, call_request, block_spec)
                    .and_then(to_json_with_traces::<_, ChainSpecT>)
            }
            MethodInvocation::EthSign(address, message)
            | MethodInvocation::PersonalSign(message, address) => {
                eth::handle_sign_request(data, message, address).and_then(to_json::<_, ChainSpecT>)
            }
            MethodInvocation::FeeHistory(block_count, newest_block, reward_percentiles) => {
                eth::handle_fee_history(data, block_count, newest_block, reward_percentiles)
                    .and_then(to_json::<_, ChainSpecT>)
            }
            MethodInvocation::GasPrice(()) => {
                eth::handle_gas_price(data).and_then(to_json::<_, ChainSpecT>)
            }
            MethodInvocation::GetBalance(address, block_spec) => {
                eth::handle_get_balance_request(data, address, block_spec)
                    .and_then(to_json::<_, ChainSpecT>)
            }
            MethodInvocation::GetBlockByNumber(block_spec, transaction_detail_flag) => {
                eth::handle_get_block_by_number_request(data, block_spec, transaction_detail_flag)
                    .and_then(to_json::<_, ChainSpecT>)
            }
            MethodInvocation::GetBlockByHash(block_hash, transaction_detail_flag) => {
                eth::handle_get_block_by_hash_request(data, block_hash, transaction_detail_flag)
                    .and_then(to_json::<_, ChainSpecT>)
            }
            MethodInvocation::GetBlockTransactionCountByHash(block_hash) => {
                eth::handle_get_block_transaction_count_by_hash_request(data, block_hash)
                    .and_then(to_json::<_, ChainSpecT>)
            }
            MethodInvocation::GetBlockTransactionCountByNumber(block_spec) => {
                eth::handle_get_block_transaction_count_by_block_number(data, block_spec)
                    .and_then(to_json::<_, ChainSpecT>)
            }
            MethodInvocation::GetCode(address, block_spec) => {
                eth::handle_get_code_request(data, address, block_spec)
                    .and_then(to_json::<_, ChainSpecT>)
            }
            MethodInvocation::GetFilterChanges(filter_id) => {
                eth::handle_get_filter_changes_request(data, filter_id)
                    .and_then(to_json::<_, ChainSpecT>)
            }
            MethodInvocation::GetFilterLogs(filter_id) => {
                eth::handle_get_filter_logs_request(data, filter_id)
                    .and_then(to_json::<_, ChainSpecT>)
            }
            MethodInvocation::GetLogs(filter_options) => {
                eth::handle_get_logs_request(data, filter_options)
                    .and_then(to_json::<_, ChainSpecT>)
            }
            MethodInvocation::GetStorageAt(address, index, block_spec) => {
                eth::handle_get_storage_at_request(data, address, index, block_spec)
                    .and_then(to_json::<_, ChainSpecT>)
            }
            MethodInvocation::GetTransactionByBlockHashAndIndex(block_hash, index) => {
                eth::handle_get_transaction_by_block_hash_and_index(data, block_hash, index)
                    .and_then(to_json::<_, ChainSpecT>)
            }
            MethodInvocation::GetTransactionByBlockNumberAndIndex(block_spec, index) => {
                eth::handle_get_transaction_by_block_spec_and_index(data, block_spec, index)
                    .and_then(to_json::<_, ChainSpecT>)
            }
            MethodInvocation::GetTransactionByHash(transaction_hash) => {
                eth::handle_get_transaction_by_hash(data, transaction_hash)
                    .and_then(to_json::<_, ChainSpecT>)
            }
            MethodInvocation::GetTransactionCount(address, block_spec) => {
                eth::handle_get_transaction_count_request(data, address, block_spec)
                    .and_then(to_json::<_, ChainSpecT>)
            }
            MethodInvocation::GetTransactionReceipt(transaction_hash) => {
                eth::handle_get_transaction_receipt(data, transaction_hash)
                    .and_then(to_json::<_, ChainSpecT>)
            }
            MethodInvocation::MaxPriorityFeePerGas(()) => {
                eth::handle_max_priority_fee_per_gas::<ChainSpecT>()
                    .and_then(to_json::<_, ChainSpecT>)
            }
            MethodInvocation::Mining(()) => {
                eth::handle_mining::<ChainSpecT>().and_then(to_json::<_, ChainSpecT>)
            }
            MethodInvocation::NetListening(()) => {
                eth::handle_net_listening_request::<ChainSpecT>().and_then(to_json::<_, ChainSpecT>)
            }
            MethodInvocation::NetPeerCount(()) => {
                eth::handle_net_peer_count_request::<ChainSpecT>()
                    .and_then(to_json::<_, ChainSpecT>)
            }
            MethodInvocation::NetVersion(()) => {
                eth::handle_net_version_request(data).and_then(to_json::<_, ChainSpecT>)
            }
            MethodInvocation::NewBlockFilter(()) => {
                eth::handle_new_block_filter_request(data).and_then(to_json::<_, ChainSpecT>)
            }
            MethodInvocation::NewFilter(options) => {
                eth::handle_new_log_filter_request(data, options).and_then(to_json::<_, ChainSpecT>)
            }
            MethodInvocation::NewPendingTransactionFilter(()) => {
                eth::handle_new_pending_transaction_filter_request(data)
                    .and_then(to_json::<_, ChainSpecT>)
            }
            MethodInvocation::PendingTransactions(()) => {
                eth::handle_pending_transactions(data).and_then(to_json::<_, ChainSpecT>)
            }
            MethodInvocation::SendRawTransaction(raw_transaction) => {
                eth::handle_send_raw_transaction_request(data, raw_transaction)
                    .and_then(to_json_with_traces::<_, ChainSpecT>)
            }
            MethodInvocation::SendTransaction(transaction_request) => {
                eth::handle_send_transaction_request(data, transaction_request)
                    .and_then(to_json_with_traces::<_, ChainSpecT>)
            }
            MethodInvocation::SignTypedDataV4(address, message) => {
                eth::handle_sign_typed_data_v4(data, address, message)
                    .and_then(to_json::<_, ChainSpecT>)
            }
            MethodInvocation::Subscribe(subscription_type, filter_options) => {
                eth::handle_subscribe_request(data, subscription_type, filter_options)
                    .and_then(to_json::<_, ChainSpecT>)
            }
            MethodInvocation::Syncing(()) => {
                eth::handle_syncing::<ChainSpecT>().and_then(to_json::<_, ChainSpecT>)
            }
            MethodInvocation::UninstallFilter(filter_id) => {
                eth::handle_uninstall_filter_request(data, filter_id)
                    .and_then(to_json::<_, ChainSpecT>)
            }
            MethodInvocation::Unsubscribe(filter_id) => {
                eth::handle_unsubscribe_request(data, filter_id).and_then(to_json::<_, ChainSpecT>)
            }

            // web3_* methods
            MethodInvocation::Web3ClientVersion(()) => {
                eth::handle_web3_client_version_request::<ChainSpecT>()
                    .and_then(to_json::<_, ChainSpecT>)
            }
            MethodInvocation::Web3Sha3(message) => {
                eth::handle_web3_sha3_request::<ChainSpecT>(message)
                    .and_then(to_json::<_, ChainSpecT>)
            }

            // evm_* methods
            MethodInvocation::EvmIncreaseTime(increment) => {
                eth::handle_increase_time_request(data, increment)
                    .and_then(to_json::<_, ChainSpecT>)
            }
            MethodInvocation::EvmMine(timestamp) => eth::handle_mine_request(data, timestamp)
                .and_then(to_json_with_traces::<_, ChainSpecT>),
            MethodInvocation::EvmRevert(snapshot_id) => {
                eth::handle_revert_request(data, snapshot_id).and_then(to_json::<_, ChainSpecT>)
            }
            MethodInvocation::EvmSetAutomine(enabled) => {
                eth::handle_set_automine_request(data, enabled).and_then(to_json::<_, ChainSpecT>)
            }
            MethodInvocation::EvmSetBlockGasLimit(gas_limit) => {
                eth::handle_set_block_gas_limit_request(data, gas_limit)
                    .and_then(to_json::<_, ChainSpecT>)
            }
            MethodInvocation::EvmSetIntervalMining(config) => handle_set_interval_mining(
                self.data.clone(),
                &mut self.interval_miner.lock(),
                self.runtime.clone(),
                config,
            )
            .and_then(to_json::<_, ChainSpecT>),
            MethodInvocation::EvmSetNextBlockTimestamp(timestamp) => {
                eth::handle_set_next_block_timestamp_request(data, timestamp)
                    .and_then(to_json::<_, ChainSpecT>)
            }
            MethodInvocation::EvmSnapshot(()) => {
                eth::handle_snapshot_request(data).and_then(to_json::<_, ChainSpecT>)
            }

            // debug_* methods
            MethodInvocation::DebugTraceTransaction(transaction_hash, config) => {
                debug::handle_debug_trace_transaction(data, transaction_hash, config)
                    .and_then(to_json_with_traces::<_, ChainSpecT>)
            }
            MethodInvocation::DebugTraceCall(call_request, block_spec, config) => {
                debug::handle_debug_trace_call(data, call_request, block_spec, config)
                    .and_then(to_json_with_traces::<_, ChainSpecT>)
            }

            // hardhat_* methods
            MethodInvocation::AddCompilationResult(
                solc_version,
                compiler_input,
                compiler_output,
            ) => hardhat::handle_add_compilation_result(
                data,
                solc_version,
                *compiler_input,
                compiler_output,
            )
            .and_then(to_json::<_, ChainSpecT>),
            MethodInvocation::DropTransaction(transaction_hash) => {
                hardhat::handle_drop_transaction(data, transaction_hash)
                    .and_then(to_json::<_, ChainSpecT>)
            }
            MethodInvocation::GetAutomine(()) => {
                hardhat::handle_get_automine_request(data).and_then(to_json::<_, ChainSpecT>)
            }
            MethodInvocation::ImpersonateAccount(address) => {
                hardhat::handle_impersonate_account_request(data, *address)
                    .and_then(to_json::<_, ChainSpecT>)
            }
            // TODO: how to return traces from interval mine to the client?
            MethodInvocation::IntervalMine(()) => {
                hardhat::handle_interval_mine_request(data).and_then(to_json::<_, ChainSpecT>)
            }
            MethodInvocation::Metadata(()) => {
                hardhat::handle_metadata_request(data).and_then(to_json::<_, ChainSpecT>)
            }
            MethodInvocation::Mine(number_of_blocks, interval) => {
                hardhat::handle_mine(data, number_of_blocks, interval)
                    .and_then(to_json_with_traces::<_, ChainSpecT>)
            }
            MethodInvocation::Reset(config) => {
                self.reset(data, config).and_then(to_json::<_, ChainSpecT>)
            }
            MethodInvocation::SetBalance(address, balance) => {
                hardhat::handle_set_balance(data, address, balance)
                    .and_then(to_json::<_, ChainSpecT>)
            }
            MethodInvocation::SetCode(address, code) => {
                hardhat::handle_set_code(data, address, code).and_then(to_json::<_, ChainSpecT>)
            }
            MethodInvocation::SetCoinbase(coinbase) => {
                hardhat::handle_set_coinbase_request(data, coinbase)
                    .and_then(to_json::<_, ChainSpecT>)
            }
            MethodInvocation::SetLoggingEnabled(is_enabled) => {
                hardhat::handle_set_logging_enabled_request(data, is_enabled)
                    .and_then(to_json::<_, ChainSpecT>)
            }
            MethodInvocation::SetMinGasPrice(min_gas_price) => {
                hardhat::handle_set_min_gas_price(data, min_gas_price.to())
                    .and_then(to_json::<_, ChainSpecT>)
            }
            MethodInvocation::SetNextBlockBaseFeePerGas(base_fee_per_gas) => {
                hardhat::handle_set_next_block_base_fee_per_gas_request(data, base_fee_per_gas.to())
                    .and_then(to_json::<_, ChainSpecT>)
            }
            MethodInvocation::SetNonce(address, nonce) => {
                hardhat::handle_set_nonce(data, address, nonce).and_then(to_json::<_, ChainSpecT>)
            }
            MethodInvocation::SetPrevRandao(prev_randao) => {
                hardhat::handle_set_prev_randao_request(data, prev_randao)
                    .and_then(to_json::<_, ChainSpecT>)
            }
            MethodInvocation::SetStorageAt(address, index, value) => {
                hardhat::handle_set_storage_at(data, address, index, value)
                    .and_then(to_json::<_, ChainSpecT>)
            }
            MethodInvocation::StopImpersonatingAccount(address) => {
                hardhat::handle_stop_impersonating_account_request(data, *address)
                    .and_then(to_json::<_, ChainSpecT>)
            }
        };

        if let Some(method_name) = method_name {
            // Skip printing for `hardhat_intervalMine` unless it is an error
            if method_name != "hardhat_intervalMine" || result.is_err() {
                data.logger_mut()
                    .print_method_logs(method_name, result.as_ref().err())
                    .map_err(ProviderError::Logger)?;
            }
        }

        result
    }

    fn reset(
        &self,
        data: &mut ProviderData<ChainSpecT, TimerT>,
        config: Option<ResetProviderConfig>,
    ) -> Result<bool, ProviderErrorForChainSpec<ChainSpecT>> {
        let mut interval_miner = self.interval_miner.lock();
        interval_miner.take();

        data.reset(config.and_then(|c| c.forking))?;

        *interval_miner = data.mining_config().interval.as_ref().map(|config| {
            IntervalMiner::new(self.runtime.clone(), config.clone(), self.data.clone())
        });

        Ok(true)
    }
}
