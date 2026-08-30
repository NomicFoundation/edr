//! Routing of JSON-RPC method invocations to their request handlers.

use edr_chain_spec::TransactionValidation;
use edr_transaction::{IsEip155, IsEip4844, TransactionMut, TransactionType};

use crate::{
    data::ProviderData,
    error::{ProviderError, ProviderErrorForChainSpec},
    requests::{debug, eth, hardhat, MethodInvocation, ProviderRequest},
    spec::SyncProviderSpec,
    time::TimeSinceEpoch,
    to_json, to_json_with_trace, to_json_with_traces, ResponseWithCallTraces, PRIVATE_RPC_METHODS,
};

/// Executes a single or batched JSON-RPC request.
pub(crate) fn execute_request<ChainSpecT, TimerT>(
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
        ProviderRequest::Single(request) => execute_single_request(data, *request),
        ProviderRequest::Batch(requests) => execute_batch_request(data, requests),
    }
}

/// Executes a batch of JSON-RPC method invocations.
fn execute_batch_request<ChainSpecT, TimerT>(
    data: &mut ProviderData<ChainSpecT, TimerT>,
    requests: Vec<MethodInvocation<ChainSpecT>>,
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
    let mut results = Vec::with_capacity(requests.len());
    let mut call_trace_arenas = Vec::new();

    for request in requests {
        let response = execute_single_request(data, request)?;

        results.push(response.result);
        call_trace_arenas.extend(response.call_trace_arenas);
    }

    Ok(ResponseWithCallTraces {
        result: to_json_array(&results),
        call_trace_arenas,
    })
}

/// Collects already-serialized JSON values into a JSON array.
fn to_json_array(values: &[Box<serde_json::value::RawValue>]) -> Box<serde_json::value::RawValue> {
    let capacity = values
        .iter()
        .map(|value| value.get().len() + 1)
        .sum::<usize>()
        + 1;

    let mut json = String::with_capacity(capacity);
    json.push('[');
    for (index, value) in values.iter().enumerate() {
        if index > 0 {
            json.push(',');
        }
        json.push_str(value.get());
    }
    json.push(']');

    // SAFETY: every element is a single well-formed JSON value, so the array is
    // one too.
    unsafe { serde_json::value::RawValue::from_string_unchecked(json) }
}

/// Executes a single JSON-RPC request, printing its method logs unless the
/// method is private.
fn execute_single_request<ChainSpecT, TimerT>(
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

    let result = route_method_invocation(data, request);

    if let Some(method_name) = method_name {
        data.logger_mut()
            .print_method_logs(method_name, result.as_ref().err())
            .map_err(ProviderError::Logger)?;
    }

    result
}

/// Routes a method invocation to its handler.
fn route_method_invocation<ChainSpecT, TimerT>(
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
    match request {
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
    }
}

#[cfg(test)]
mod tests {
    use edr_chain_l1::L1ChainSpec;
    use edr_primitives::U256;

    use super::*;
    use crate::test_utils::ProviderTestFixture;

    #[test]
    fn execute_batch_request_preserves_order() -> anyhow::Result<()> {
        let mut fixture = ProviderTestFixture::<L1ChainSpec>::new_local()?;

        let response = execute_request(
            &mut fixture.provider_data,
            ProviderRequest::Batch(vec![
                MethodInvocation::ChainId(()),
                MethodInvocation::BlockNumber(()),
                MethodInvocation::NetVersion(()),
            ]),
        )?;

        let results: Vec<Box<serde_json::value::RawValue>> = response.deserialize_result()?;
        assert_eq!(results.len(), 3);

        let chain_id: U256 = serde_json::from_str(results[0].get())?;
        assert_eq!(chain_id.to::<u64>(), fixture.config.chain_id);

        let block_number: U256 = serde_json::from_str(results[1].get())?;
        assert_eq!(
            block_number.to::<u64>(),
            fixture.provider_data.last_block_number()
        );

        let network_id: String = serde_json::from_str(results[2].get())?;
        assert_eq!(network_id, fixture.config.network_id.to_string());

        Ok(())
    }

    #[test]
    fn execute_batch_request_is_empty_for_no_requests() -> anyhow::Result<()> {
        let mut fixture = ProviderTestFixture::<L1ChainSpec>::new_local()?;

        let response = execute_request(
            &mut fixture.provider_data,
            ProviderRequest::Batch(Vec::new()),
        )?;

        assert_eq!(response.result.get(), "[]");
        assert!(response.call_trace_arenas.is_empty());

        Ok(())
    }
}
