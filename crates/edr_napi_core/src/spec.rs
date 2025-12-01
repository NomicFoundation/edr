use std::sync::Arc;

use edr_chain_l1::L1ChainSpec;
use edr_chain_spec::{EvmHaltReason, HaltReasonTrait, TransactionValidation};
use edr_generic::GenericChainSpec;
use edr_provider::{
    time::TimeSinceEpoch, ProviderErrorForChainSpec, ResponseWithCallTraces, SyncProviderSpec,
};
use edr_rpc_client::jsonrpc;
use edr_solidity::contract_decoder::ContractDecoder;
use edr_solidity_tests::{
    executors::stack_trace::{get_stack_trace, SolidityTestStackTraceResult},
    traces::SparsedTraceArena,
};
use edr_tracing::Trace;
use edr_transaction::{IsEip155, IsEip4844, TransactionMut, TransactionType};
use napi::{Either, Status};

pub type ResponseData = Either<String, serde_json::Value>;

pub struct Response {
    // N-API is known to be slow when marshalling `serde_json::Value`s, so we try to return a
    // `String`. If the object is too large to be represented as a `String`, we return a `Buffer`
    // instead.
    pub data: ResponseData,
    /// When a transaction fails to execute, the provider returns a stack trace
    /// of the transaction.
    ///
    /// If the heuristic failed the vec is set but emtpy.
    /// Error if there was an error computing the stack trace.
    pub solidity_trace: Option<SolidityTestStackTraceResult<String>>,
    /// This may contain zero or more traces, depending on the (batch) request
    pub traces: Vec<SparsedTraceArena>,
}

impl From<String> for Response {
    fn from(value: String) -> Self {
        Response {
            data: Either::A(value),
            solidity_trace: None,
            traces: Vec::new(),
        }
    }
}

/// Trait for a defining a chain's associated type in the N-API.
pub trait SyncNapiSpec<TimerT: Clone + TimeSinceEpoch>:
    SyncProviderSpec<
    TimerT,
    PooledTransaction: IsEip155,
    SignedTransaction: Default
                           + TransactionMut
                           + TransactionType<Type: IsEip4844>
                           + TransactionValidation<ValidationError: PartialEq>,
>
{
    /// The string type identifier of the chain.
    const CHAIN_TYPE: &'static str;

    /// Casts a response with traces into a `Response`.
    ///
    /// This is implemented as an associated function to avoid problems when
    /// implementing type conversions for third-party types.
    fn cast_response(
        response: Result<ResponseWithCallTraces<Self::HaltReason>, ProviderErrorForChainSpec<Self>>,
        contract_decoder: Arc<ContractDecoder>,
    ) -> napi::Result<Response<EvmHaltReason>>;
}

impl<TimerT: Clone + TimeSinceEpoch> SyncNapiSpec<TimerT> for L1ChainSpec {
    const CHAIN_TYPE: &'static str = edr_chain_l1::CHAIN_TYPE;

    fn cast_response(
        mut response: Result<
            ResponseWithCallTraces<Self::HaltReason>,
            ProviderErrorForChainSpec<Self>,
        >,
        contract_decoder: Arc<ContractDecoder>,
    ) -> napi::Result<Response<EvmHaltReason>> {
        // We can take the solidity trace as it won't be used for anything else
        let solidity_trace = response.as_mut().err().and_then(|error| {
            if let edr_provider::ProviderError::TransactionFailed(failure) = error {
                if matches!(
                    failure.failure.reason,
                    edr_provider::TransactionFailureReason::OutOfGas(_)
                ) {
                    None
                } else {
                    let trace = std::mem::take(&mut failure.failure.solidity_trace);

                    let result =
                        SolidityTestStackTraceResult::from(get_stack_trace::<edr_chain_l1::HaltReason, _>(
                            contract_decoder.as_ref(),
                            &[(TraceKind::Execution, trace)],
                        ));

                    let result = result.map_halt_reason(|halt_reason: HaltReasonT| {
                        serde_json::to_string(&halt_reason)
                            .expect("Failed to serialize halt reason")
                    });

                    Some(result)
                }
            } else {
                None
            }
        });

        // We can take the traces as they won't be used for anything else
        let traces = match &mut response {
            Ok(response) => std::mem::take(&mut response.call_traces),
            Err(edr_provider::ProviderError::TransactionFailed(failure)) => {
                std::mem::take(&mut failure.call_traces)
            }
            Err(_) => Vec::new(),
        };

        let response = jsonrpc::ResponseData::from(response.map(|response| response.result));

        marshal_response_data(response).map(|data| Response {
            solidity_trace,
            data,
            traces,
        })
    }
}

impl<TimerT: Clone + TimeSinceEpoch> SyncNapiSpec<TimerT> for GenericChainSpec {
    const CHAIN_TYPE: &'static str = edr_generic::CHAIN_TYPE;

    fn cast_response(
        mut response: Result<
            ResponseWithCallTraces<Self::HaltReason>,
            ProviderErrorForChainSpec<Self>,
        >,
        contract_decoder: Arc<ContractDecoder>,
    ) -> napi::Result<Response<EvmHaltReason>> {
        // We can take the solidity trace as it won't be used for anything else
        let solidity_trace: Option<Arc<Trace<EvmHaltReason>>> =
            response.as_mut().err().and_then(|error| {
                if let edr_provider::ProviderError::TransactionFailed(failure) = error {
                    if matches!(
                        failure.failure.reason,
                        edr_provider::TransactionFailureReason::OutOfGas(_)
                    ) {
                        None
                    } else {
                        let trace = std::mem::take(&mut failure.failure.solidity_trace);

                        let result =
                            SolidityTestStackTraceResult::from(get_stack_trace::<edr_chain_l1::HaltReason, _>(
                                contract_decoder.as_ref(),
                                &[(TraceKind::Execution, trace)],
                            ));

                        let result = result.map_halt_reason(|halt_reason: HaltReasonT| {
                            serde_json::to_string(&halt_reason)
                                .expect("Failed to serialize halt reason")
                        });

                        Some(result)
                    }
                } else {
                    None
                }
            });

        // We can take the traces as they won't be used for anything else
        let traces = match &mut response {
            Ok(response) => std::mem::take(&mut response.call_traces),
            Err(edr_provider::ProviderError::TransactionFailed(failure)) => {
                std::mem::take(&mut failure.call_traces)
            }
            Err(_) => Vec::new(),
        };

        let response = jsonrpc::ResponseData::from(response.map(|response| response.result));

        marshal_response_data(response).map(|data| Response {
            solidity_trace,
            data,
            traces,
        })
    }
}

/// Marshals a JSON-RPC response data into a `ResponseData`, taking into account
/// large responses.
pub fn marshal_response_data(
    response: jsonrpc::ResponseData<serde_json::Value>,
) -> napi::Result<ResponseData> {
    serde_json::to_string(&response)
        .and_then(|json| {
            // We experimentally determined that 500_000_000 was the maximum string length
            // that can be returned without causing the error:
            //
            // > Failed to convert rust `String` into napi `string`
            //
            // To be safe, we're limiting string lengths to half of that.
            const MAX_STRING_LENGTH: usize = 250_000_000;

            if json.len() <= MAX_STRING_LENGTH {
                Ok(Either::A(json))
            } else {
                serde_json::to_value(response).map(Either::B)
            }
        })
        .map_err(|error| napi::Error::new(Status::GenericFailure, error.to_string()))
}
