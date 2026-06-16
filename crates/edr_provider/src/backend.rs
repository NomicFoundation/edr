use std::{
    convert::Infallible,
    sync::Arc,
    time::{Duration, Instant},
};

use crossbeam_channel::{Receiver, Sender};
use edr_chain_spec::TransactionValidation;
use edr_chain_spec_provider::ProviderChainSpec;
use edr_transaction::{IsEip155, IsEip4844, TransactionMut, TransactionType};

use crate::{
    data::ProviderData,
    error::{ProviderError, ProviderErrorForChainSpec},
    mock::SyncCallOverride,
    requests::{debug, eth, hardhat, MethodInvocation, ProviderRequest},
    spec::SyncProviderSpec,
    time::TimeSinceEpoch,
    to_json, to_json_with_trace, to_json_with_traces, ResponseWithCallTraces, PRIVATE_RPC_METHODS,
};

/// The response to a [`BackendRequest::Request`].
pub(crate) type RequestResponse<ChainSpecT> =
    Result<ResponseWithCallTraces, ProviderErrorForChainSpec<ChainSpecT>>;

/// A message processed by the provider's background thread.
///
/// The thread owns the [`ProviderData`] outright; all access goes through these
/// messages so that requests and interval mining are serialized on a single
/// thread without any locking.
pub(crate) enum BackendRequest<ChainSpecT: ProviderChainSpec> {
    /// Handle a single or batched JSON-RPC request, returning the response on
    /// `response_sender`.
    Request {
        request: ProviderRequest<ChainSpecT>,
        response_sender: Sender<RequestResponse<ChainSpecT>>,
    },
    /// Set (or clear) the call-override callback.
    SetCallOverrideCallback {
        callback: Option<Arc<dyn SyncCallOverride>>,
        ack: Sender<()>,
    },
    /// Toggle whether traces include the full stack and memory.
    SetVerboseTracing { enabled: bool, ack: Sender<()> },
    /// Log a failed request deserialization through the provider's logger.
    LogFailedDeserialization {
        method_name: String,
        error: Box<ProviderErrorForChainSpec<ChainSpecT>>,
        ack: Sender<Result<(), ProviderErrorForChainSpec<ChainSpecT>>>,
    },
}

/// Creates a channel that yields a message whenever the next interval-mined
/// block is due, if interval mining is enabled. Otherwise, creates a channel
/// that never yields.
fn next_interval_timer(
    interval_config: Option<&crate::config::IntervalConfig>,
) -> Receiver<Instant> {
    if let Some(config) = interval_config {
        let duration = Duration::from_millis(config.generate_interval());
        crossbeam_channel::after(duration)
    } else {
        crossbeam_channel::never()
    }
}

/// The event loop run by the provider's dedicated background thread.
///
/// It processes incoming requests in order while giving interval mining
/// precedence whenever a block is due. The loop owns `data` and runs until the
/// `cancellation_receiver` is disconnected (by [`crate::Provider`]'s `Drop`
/// dropping the matching sender), or all request senders are dropped.
pub(super) fn run<ChainSpecT, TimerT>(
    mut data: ProviderData<ChainSpecT, TimerT>,
    request_receiver: Receiver<BackendRequest<ChainSpecT>>,
    cancellation_receiver: Receiver<Infallible>,
) where
    ChainSpecT: SyncProviderSpec<
        TimerT,
        PooledTransaction: IsEip155,
        SignedTransaction: Default
                               + TransactionMut
                               + TransactionType<Type: IsEip4844>
                               + TransactionValidation<ValidationError: PartialEq>,
    >,
    TimerT: Clone + TimeSinceEpoch,
{
    let mut interval_timer = next_interval_timer(data.interval_config());

    loop {
        crossbeam_channel::select_biased! {
            // Highest priority. The cancellation channel carries `Infallible`, so
            // the only event it can ever yield is disconnection, signalled by
            // `Provider::drop` (which runs off the JS thread via the N-API
            // AsyncDeallocator).
            recv(cancellation_receiver) -> _ => break,
            // Interval mining takes precedence over incoming requests. An overdue
            // deadline yields a zero duration, so `after` is immediately ready.
            recv(interval_timer) -> _ => {
                if let Err(error) = data.interval_mine() {
                    log::error!("Unexpected error while performing interval mining: {error}");
                }
                interval_timer = next_interval_timer(data.interval_config());
            }
            recv(request_receiver) -> message => match message {
                Ok(BackendRequest::Request { request, response_sender }) => {
                    let current_interval = data.interval_config().cloned();

                    let response = handle_request(&mut data, request);

                    // Ignore the error: the caller may have stopped waiting.
                    let _ = response_sender.send(response);

                    // `evm_setIntervalMining` may have changed the configuration.
                    if data.interval_config() != current_interval.as_ref() {
                        interval_timer = next_interval_timer(data.interval_config());
                    }
                }
                Ok(BackendRequest::SetCallOverrideCallback { callback, ack }) => {
                    data.set_call_override_callback(callback);

                    // Ignore the error: the caller may have stopped waiting.
                    let _ = ack.send(());
                }
                Ok(BackendRequest::SetVerboseTracing { enabled, ack }) => {
                    data.set_verbose_tracing(enabled);

                    // Ignore the error: the caller may have stopped waiting.
                    let _ = ack.send(());
                }
                Ok(BackendRequest::LogFailedDeserialization { method_name, error, ack }) => {
                    let result = data
                        .logger_mut()
                        .print_method_logs(&method_name, Some(&error))
                        .map_err(ProviderError::Logger);

                    // Ignore the error: the caller may have stopped waiting.
                    let _ = ack.send(result);
                }
                // All request senders were dropped — backstop in case the
                // shutdown signal is not used.
                Err(_) => break,
            }
        }
    }
}

/// Handles a single or batched JSON-RPC request.
fn handle_request<ChainSpecT, TimerT>(
    data: &mut ProviderData<ChainSpecT, TimerT>,
    request: ProviderRequest<ChainSpecT>,
) -> Result<ResponseWithCallTraces, ProviderErrorForChainSpec<ChainSpecT>>
where
    ChainSpecT: SyncProviderSpec<
        TimerT,
        PooledTransaction: IsEip155,
        SignedTransaction: Default
                               + TransactionMut
                               + TransactionType<Type: IsEip4844>
                               + TransactionValidation<ValidationError: PartialEq>,
    >,
    TimerT: Clone + TimeSinceEpoch,
{
    match request {
        ProviderRequest::Single(request) => handle_single_request(data, *request),
        ProviderRequest::Batch(requests) => handle_batch_request(data, requests),
    }
}

/// Handles a batch of JSON requests for an execution provider.
fn handle_batch_request<ChainSpecT, TimerT>(
    data: &mut ProviderData<ChainSpecT, TimerT>,
    request: Vec<MethodInvocation<ChainSpecT>>,
) -> Result<ResponseWithCallTraces, ProviderErrorForChainSpec<ChainSpecT>>
where
    ChainSpecT: SyncProviderSpec<
        TimerT,
        PooledTransaction: IsEip155,
        SignedTransaction: Default
                               + TransactionMut
                               + TransactionType<Type: IsEip4844>
                               + TransactionValidation<ValidationError: PartialEq>,
    >,
    TimerT: Clone + TimeSinceEpoch,
{
    let mut results = Vec::new();
    let mut traces = Vec::new();

    for req in request {
        let response = handle_single_request(data, req)?;
        results.push(response.result);
        traces.extend(response.call_trace_arenas);
    }

    let result = serde_json::to_value(results).map_err(ProviderError::Serialization)?;
    Ok(ResponseWithCallTraces {
        result,
        call_trace_arenas: traces,
    })
}

fn handle_single_request<ChainSpecT, TimerT>(
    data: &mut ProviderData<ChainSpecT, TimerT>,
    request: MethodInvocation<ChainSpecT>,
) -> Result<ResponseWithCallTraces, ProviderErrorForChainSpec<ChainSpecT>>
where
    ChainSpecT: SyncProviderSpec<
        TimerT,
        PooledTransaction: IsEip155,
        SignedTransaction: Default
                               + TransactionMut
                               + TransactionType<Type: IsEip4844>
                               + TransactionValidation<ValidationError: PartialEq>,
    >,
    TimerT: Clone + TimeSinceEpoch,
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
            eth::handle_accounts_request(data).and_then(to_json::<_, ChainSpecT, TimerT>)
        }
        MethodInvocation::BlobBaseFee(()) => {
            eth::handle_blob_base_fee(data).and_then(to_json::<_, ChainSpecT, TimerT>)
        }
        MethodInvocation::BlockNumber(()) => {
            eth::handle_block_number_request(data).and_then(to_json::<_, ChainSpecT, TimerT>)
        }
        MethodInvocation::Call(request, block_spec, state_overrides) => {
            eth::handle_call_request(data, request, block_spec, state_overrides)
                .and_then(to_json_with_trace::<_, ChainSpecT, TimerT>)
        }
        MethodInvocation::ChainId(()) => {
            eth::handle_chain_id_request(data).and_then(to_json::<_, ChainSpecT, TimerT>)
        }
        MethodInvocation::Coinbase(()) => {
            eth::handle_coinbase_request(data).and_then(to_json::<_, ChainSpecT, TimerT>)
        }
        MethodInvocation::EstimateGas(call_request, block_spec) => {
            eth::handle_estimate_gas(data, call_request, block_spec)
                .and_then(to_json_with_traces::<_, ChainSpecT, TimerT>)
        }
        MethodInvocation::EthSign(address, message)
        | MethodInvocation::PersonalSign(message, address) => {
            eth::handle_sign_request(data, message, address)
                .and_then(to_json::<_, ChainSpecT, TimerT>)
        }
        MethodInvocation::FeeHistory(block_count, newest_block, reward_percentiles) => {
            eth::handle_fee_history(data, block_count, newest_block, reward_percentiles)
                .and_then(to_json::<_, ChainSpecT, TimerT>)
        }
        MethodInvocation::GasPrice(()) => {
            eth::handle_gas_price(data).and_then(to_json::<_, ChainSpecT, TimerT>)
        }
        MethodInvocation::GetBalance(address, block_spec) => {
            eth::handle_get_balance_request(data, address, block_spec)
                .and_then(to_json::<_, ChainSpecT, TimerT>)
        }
        MethodInvocation::GetBlockByNumber(block_spec, transaction_detail_flag) => {
            eth::handle_get_block_by_number_request(data, block_spec, transaction_detail_flag)
                .and_then(to_json::<_, ChainSpecT, TimerT>)
        }
        MethodInvocation::GetBlockByHash(block_hash, transaction_detail_flag) => {
            eth::handle_get_block_by_hash_request(data, block_hash, transaction_detail_flag)
                .and_then(to_json::<_, ChainSpecT, TimerT>)
        }
        MethodInvocation::GetBlockTransactionCountByHash(block_hash) => {
            eth::handle_get_block_transaction_count_by_hash_request(data, block_hash)
                .and_then(to_json::<_, ChainSpecT, TimerT>)
        }
        MethodInvocation::GetBlockTransactionCountByNumber(block_spec) => {
            eth::handle_get_block_transaction_count_by_block_number(data, block_spec)
                .and_then(to_json::<_, ChainSpecT, TimerT>)
        }
        MethodInvocation::GetCode(address, block_spec) => {
            eth::handle_get_code_request(data, address, block_spec)
                .and_then(to_json::<_, ChainSpecT, TimerT>)
        }
        MethodInvocation::GetFilterChanges(filter_id) => {
            eth::handle_get_filter_changes_request(data, filter_id)
                .and_then(to_json::<_, ChainSpecT, TimerT>)
        }
        MethodInvocation::GetFilterLogs(filter_id) => {
            eth::handle_get_filter_logs_request(data, filter_id)
                .and_then(to_json::<_, ChainSpecT, TimerT>)
        }
        MethodInvocation::GetLogs(filter_options) => {
            eth::handle_get_logs_request(data, filter_options)
                .and_then(to_json::<_, ChainSpecT, TimerT>)
        }
        MethodInvocation::GetProof(address, storage_keys, block_spec) => {
            eth::handle_get_proof_request(data, address, storage_keys, block_spec)
                .and_then(to_json::<_, ChainSpecT, TimerT>)
        }
        MethodInvocation::GetStorageAt(address, index, block_spec) => {
            eth::handle_get_storage_at_request(data, address, index, block_spec)
                .and_then(to_json::<_, ChainSpecT, TimerT>)
        }
        MethodInvocation::GetTransactionByBlockHashAndIndex(block_hash, index) => {
            eth::handle_get_transaction_by_block_hash_and_index(data, block_hash, index)
                .and_then(to_json::<_, ChainSpecT, TimerT>)
        }
        MethodInvocation::GetTransactionByBlockNumberAndIndex(block_spec, index) => {
            eth::handle_get_transaction_by_block_spec_and_index(data, block_spec, index)
                .and_then(to_json::<_, ChainSpecT, TimerT>)
        }
        MethodInvocation::GetTransactionByHash(transaction_hash) => {
            eth::handle_get_transaction_by_hash(data, transaction_hash)
                .and_then(to_json::<_, ChainSpecT, TimerT>)
        }
        MethodInvocation::GetTransactionCount(address, block_spec) => {
            eth::handle_get_transaction_count_request(data, address, block_spec)
                .and_then(to_json::<_, ChainSpecT, TimerT>)
        }
        MethodInvocation::GetTransactionReceipt(transaction_hash) => {
            eth::handle_get_transaction_receipt(data, transaction_hash)
                .and_then(to_json::<_, ChainSpecT, TimerT>)
        }
        MethodInvocation::MaxPriorityFeePerGas(()) => {
            eth::handle_max_priority_fee_per_gas::<ChainSpecT, TimerT>()
                .and_then(to_json::<_, ChainSpecT, TimerT>)
        }
        MethodInvocation::NetVersion(()) => {
            eth::handle_net_version_request(data).and_then(to_json::<_, ChainSpecT, TimerT>)
        }
        MethodInvocation::NewBlockFilter(()) => {
            eth::handle_new_block_filter_request(data).and_then(to_json::<_, ChainSpecT, TimerT>)
        }
        MethodInvocation::NewFilter(options) => eth::handle_new_log_filter_request(data, options)
            .and_then(to_json::<_, ChainSpecT, TimerT>),
        MethodInvocation::NewPendingTransactionFilter(()) => {
            eth::handle_new_pending_transaction_filter_request(data)
                .and_then(to_json::<_, ChainSpecT, TimerT>)
        }
        MethodInvocation::PendingTransactions(()) => {
            eth::handle_pending_transactions(data).and_then(to_json::<_, ChainSpecT, TimerT>)
        }
        MethodInvocation::SendRawTransaction(raw_transaction) => {
            eth::handle_send_raw_transaction_request(data, raw_transaction)
                .and_then(to_json_with_traces::<_, ChainSpecT, TimerT>)
        }
        MethodInvocation::SendTransaction(transaction_request) => {
            eth::handle_send_transaction_request(data, transaction_request)
                .and_then(to_json_with_traces::<_, ChainSpecT, TimerT>)
        }
        MethodInvocation::SignTypedDataV4(address, message) => {
            eth::handle_sign_typed_data_v4(data, address, message)
                .and_then(to_json::<_, ChainSpecT, TimerT>)
        }
        MethodInvocation::Subscribe(subscription_type, filter_options) => {
            eth::handle_subscribe_request(data, subscription_type, filter_options)
                .and_then(to_json::<_, ChainSpecT, TimerT>)
        }
        MethodInvocation::Syncing(()) => {
            eth::handle_syncing::<ChainSpecT, TimerT>().and_then(to_json::<_, ChainSpecT, TimerT>)
        }
        MethodInvocation::UninstallFilter(filter_id) => {
            eth::handle_uninstall_filter_request(data, filter_id)
                .and_then(to_json::<_, ChainSpecT, TimerT>)
        }
        MethodInvocation::Unsubscribe(filter_id) => {
            eth::handle_unsubscribe_request(data, filter_id)
                .and_then(to_json::<_, ChainSpecT, TimerT>)
        }

        // web3_* methods
        MethodInvocation::Web3ClientVersion(()) => {
            eth::handle_web3_client_version_request::<ChainSpecT, TimerT>()
                .and_then(to_json::<_, ChainSpecT, TimerT>)
        }
        MethodInvocation::Web3Sha3(message) => {
            eth::handle_web3_sha3_request::<ChainSpecT, TimerT>(message)
                .and_then(to_json::<_, ChainSpecT, TimerT>)
        }

        // evm_* methods
        MethodInvocation::EvmIncreaseTime(increment) => {
            eth::handle_increase_time_request(data, increment)
                .and_then(to_json::<_, ChainSpecT, TimerT>)
        }
        MethodInvocation::EvmMine(timestamp) => eth::handle_mine_request(data, timestamp)
            .and_then(to_json_with_traces::<_, ChainSpecT, TimerT>),
        MethodInvocation::EvmRevert(snapshot_id) => {
            eth::handle_revert_request(data, snapshot_id).and_then(to_json::<_, ChainSpecT, TimerT>)
        }
        MethodInvocation::EvmSetAutomine(enabled) => {
            eth::handle_set_automine_request(data, enabled)
                .and_then(to_json::<_, ChainSpecT, TimerT>)
        }
        MethodInvocation::EvmSetBlockGasLimit(gas_limit) => {
            eth::handle_set_block_gas_limit_request(data, gas_limit)
                .and_then(to_json::<_, ChainSpecT, TimerT>)
        }
        MethodInvocation::EvmSetIntervalMining(config) => {
            eth::handle_set_interval_mining(data, config).and_then(to_json::<_, ChainSpecT, TimerT>)
        }
        MethodInvocation::EvmSetNextBlockTimestamp(timestamp) => {
            eth::handle_set_next_block_timestamp_request(data, timestamp)
                .and_then(to_json::<_, ChainSpecT, TimerT>)
        }
        MethodInvocation::EvmSnapshot(()) => {
            eth::handle_snapshot_request(data).and_then(to_json::<_, ChainSpecT, TimerT>)
        }

        // debug_* methods
        MethodInvocation::DebugTraceTransaction(transaction_hash, config) => {
            debug::handle_debug_trace_transaction(data, transaction_hash, config)
                .and_then(to_json_with_traces::<_, ChainSpecT, TimerT>)
        }
        MethodInvocation::DebugTraceCall(call_request, block_spec, config) => {
            debug::handle_debug_trace_call(data, call_request, block_spec, config)
                .and_then(to_json_with_traces::<_, ChainSpecT, TimerT>)
        }

        // hardhat_* methods
        MethodInvocation::DropTransaction(transaction_hash) => {
            hardhat::handle_drop_transaction(data, transaction_hash)
                .and_then(to_json::<_, ChainSpecT, TimerT>)
        }
        MethodInvocation::GetAutomine(()) => {
            hardhat::handle_get_automine_request(data).and_then(to_json::<_, ChainSpecT, TimerT>)
        }
        MethodInvocation::ImpersonateAccount(address) => {
            hardhat::handle_impersonate_account_request(data, *address)
                .and_then(to_json::<_, ChainSpecT, TimerT>)
        }
        MethodInvocation::Metadata(()) => {
            hardhat::handle_metadata_request(data).and_then(to_json::<_, ChainSpecT, TimerT>)
        }
        MethodInvocation::Mine(number_of_blocks, interval) => {
            hardhat::handle_mine(data, number_of_blocks, interval)
                .and_then(to_json_with_traces::<_, ChainSpecT, TimerT>)
        }
        MethodInvocation::SetBalance(address, balance) => {
            hardhat::handle_set_balance(data, address, balance)
                .and_then(to_json::<_, ChainSpecT, TimerT>)
        }
        MethodInvocation::SetCode(address, code) => {
            hardhat::handle_set_code(data, address, code).and_then(to_json::<_, ChainSpecT, TimerT>)
        }
        MethodInvocation::SetCoinbase(coinbase) => {
            hardhat::handle_set_coinbase_request(data, coinbase)
                .and_then(to_json::<_, ChainSpecT, TimerT>)
        }
        MethodInvocation::SetLoggingEnabled(is_enabled) => {
            hardhat::handle_set_logging_enabled_request(data, is_enabled)
                .and_then(to_json::<_, ChainSpecT, TimerT>)
        }
        MethodInvocation::SetMinGasPrice(min_gas_price) => {
            hardhat::handle_set_min_gas_price(data, min_gas_price.to())
                .and_then(to_json::<_, ChainSpecT, TimerT>)
        }
        MethodInvocation::SetNextBlockBaseFeePerGas(base_fee_per_gas) => {
            hardhat::handle_set_next_block_base_fee_per_gas_request(data, base_fee_per_gas.to())
                .and_then(to_json::<_, ChainSpecT, TimerT>)
        }
        MethodInvocation::SetNonce(address, nonce) => {
            hardhat::handle_set_nonce(data, address, nonce)
                .and_then(to_json::<_, ChainSpecT, TimerT>)
        }
        MethodInvocation::SetPrevRandao(prev_randao) => {
            hardhat::handle_set_prev_randao_request(data, prev_randao)
                .and_then(to_json::<_, ChainSpecT, TimerT>)
        }
        MethodInvocation::SetStorageAt(address, index, value) => {
            hardhat::handle_set_storage_at(data, address, index, value)
                .and_then(to_json::<_, ChainSpecT, TimerT>)
        }
        MethodInvocation::StopImpersonatingAccount(address) => {
            hardhat::handle_stop_impersonating_account_request(data, *address)
                .and_then(to_json::<_, ChainSpecT, TimerT>)
        }
    };

    if let Some(method_name) = method_name {
        data.logger_mut()
            .print_method_logs(method_name, result.as_ref().err())
            .map_err(ProviderError::Logger)?;
    }

    result
}
