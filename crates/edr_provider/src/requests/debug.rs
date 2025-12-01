use alloy_rpc_types_trace::geth::{GethDebugTracingOptions, GethTrace};
use edr_chain_spec::TransactionValidation;
use edr_eth::BlockSpec;
use edr_primitives::B256;
use edr_runtime::overrides::StateOverrides;

use crate::{
    data::ProviderData,
    debug_trace::DebugTraceResultWithCallTraces,
    requests::eth::{resolve_block_spec_for_call_request, resolve_call_request},
    spec::SyncProviderSpec,
    time::TimeSinceEpoch,
    ProviderError, ProviderResultWithCallTraces,
};

pub fn handle_debug_trace_transaction<
    ChainSpecT: SyncProviderSpec<
        TimerT,
        SignedTransaction: Default + TransactionValidation<ValidationError: PartialEq>,
    >,
    TimerT: Clone + TimeSinceEpoch,
>(
    data: &mut ProviderData<ChainSpecT, TimerT>,
    transaction_hash: B256,
    tracing_options: Option<GethDebugTracingOptions>,
) -> ProviderResultWithTraces<GethTrace, ChainSpecT> {
    let DebugTraceResultWithCallTraces {
        result,
        call_traces,
    } = data
        .debug_trace_transaction(&transaction_hash, tracing_options.unwrap_or_default())
        .map_err(|error| match error {
            ProviderError::InvalidTransactionHash(tx_hash) => ProviderError::InvalidInput(format!(
                "Unable to find a block containing transaction {tx_hash}"
            )),
            _ => error,
        })?;

    Ok((result, call_traces))
}

pub fn handle_debug_trace_call<ChainSpecT, TimerT>(
    data: &mut ProviderData<ChainSpecT, TimerT>,
    call_request: ChainSpecT::RpcCallRequest,
    block_spec: Option<BlockSpec>,
    tracing_options: Option<GethDebugTracingOptions>,
) -> ProviderResultWithCallTraces<GethTrace, ChainSpecT>
where
    ChainSpecT: SyncProviderSpec<
        TimerT,
        SignedTransaction: Default + TransactionValidation<ValidationError: PartialEq>,
    >,
    TimerT: Clone + TimeSinceEpoch,
{
    let block_spec = resolve_block_spec_for_call_request(block_spec);

    let transaction =
        resolve_call_request(data, call_request, &block_spec, &StateOverrides::default())?;

    let DebugTraceResultWithCallTraces {
        result,
        call_traces: traces,
    } = data.debug_trace_call(
        transaction,
        &block_spec,
        tracing_options.unwrap_or_default(),
    )?;

    Ok((result, traces))
}
