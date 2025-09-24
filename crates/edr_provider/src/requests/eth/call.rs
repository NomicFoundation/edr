use edr_eth::BlockSpec;
use edr_evm::{state::StateOverrides, trace::Trace, transaction};
use edr_evm_spec::{EvmTransactionValidationError, TransactionValidation};
use edr_primitives::Bytes;
use edr_rpc_eth::StateOverrideOptions;
use edr_signer::FakeSign as _;

use crate::{
    data::ProviderData,
    error::{ProviderErrorForChainSpec, TransactionFailureWithTraces},
    spec::{CallContext, FromRpcType, MaybeSender as _, SyncProviderSpec},
    time::TimeSinceEpoch,
    ProviderError, TransactionFailure,
};

pub fn handle_call_request<
    ChainSpecT: SyncProviderSpec<
        TimerT,
        BlockEnv: Default,
        SignedTransaction: Clone
                               + Default
                               + TransactionValidation<
            ValidationError: From<EvmTransactionValidationError> + PartialEq,
        >,
    >,
    TimerT: Clone + TimeSinceEpoch,
>(
    data: &mut ProviderData<ChainSpecT, TimerT>,
    request: ChainSpecT::RpcCallRequest,
    block_spec: Option<BlockSpec>,
    state_overrides: Option<StateOverrideOptions>,
) -> Result<(Bytes, Trace<ChainSpecT::HaltReason>), ProviderErrorForChainSpec<ChainSpecT>> {
    let block_spec = resolve_block_spec_for_call_request(block_spec);

    let state_overrides =
        state_overrides.map_or(Ok(StateOverrides::default()), StateOverrides::try_from)?;

    let transaction = resolve_call_request(data, request, &block_spec, &state_overrides)?;
    let result = data.run_call(transaction.clone(), &block_spec, &state_overrides)?;

    let hardfork = data.hardfork();
    data.logger_mut()
        .log_call(hardfork, &transaction, &result)
        .map_err(ProviderError::Logger)?;

    if data.bail_on_call_failure() {
        if let Some(failure) = TransactionFailure::from_execution_result::<ChainSpecT, TimerT>(
            &result.execution_result,
            None,
            &result.trace,
        ) {
            return Err(ProviderError::TransactionFailed(Box::new(
                TransactionFailureWithTraces {
                    failure,
                    traces: vec![result.trace],
                },
            )));
        }
    }

    let output = result.execution_result.into_output().unwrap_or_default();
    Ok((output, result.trace))
}

pub(crate) fn resolve_block_spec_for_call_request(block_spec: Option<BlockSpec>) -> BlockSpec {
    block_spec.unwrap_or_else(BlockSpec::latest)
}

pub(crate) fn resolve_call_request<
    ChainSpecT: SyncProviderSpec<
        TimerT,
        BlockEnv: Default,
        SignedTransaction: Default
                               + TransactionValidation<
            ValidationError: From<EvmTransactionValidationError> + PartialEq,
        >,
    >,
    TimerT: Clone + TimeSinceEpoch,
>(
    data: &mut ProviderData<ChainSpecT, TimerT>,
    request: ChainSpecT::RpcCallRequest,
    block_spec: &BlockSpec,
    state_overrides: &StateOverrides,
) -> Result<ChainSpecT::SignedTransaction, ProviderErrorForChainSpec<ChainSpecT>> {
    let sender = request
        .maybe_sender()
        .copied()
        .unwrap_or_else(|| data.default_caller());

    let context = CallContext {
        data,
        block_spec,
        state_overrides,
        default_gas_price_fn: |_data| Ok(0),
        max_fees_fn: |_data, _block_spec, max_fee_per_gas, max_priority_fee_per_gas| {
            let max_fee_per_gas = max_fee_per_gas.or(max_priority_fee_per_gas).unwrap_or(0);

            let max_priority_fee_per_gas = max_priority_fee_per_gas.unwrap_or(0);

            Ok((max_fee_per_gas, max_priority_fee_per_gas))
        },
    };

    let request = ChainSpecT::TransactionRequest::from_rpc_type(request, context)?;
    let transaction = request.fake_sign(sender);

    let hardfork = data.hardfork_at_block_spec(block_spec)?;
    transaction::validate(transaction, hardfork.into())
        .map_err(ProviderError::TransactionCreationError)
}
