use edr_chain_l1::L1ChainSpec;
use edr_chain_spec::TransactionValidation;
use edr_generic::GenericChainSpec;
use edr_provider::{
    time::TimeSinceEpoch, ProviderErrorForChainSpec, ResponseWithCallTraces, SyncProviderSpec,
};
use edr_rpc_client::jsonrpc;
use edr_solidity::solidity_stack_trace::StackTraceCreationResult;
use edr_solidity_tests::traces::CallTraceArena;
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
    /// If the heuristic failed the vec is set but empty.
    /// Error if there was an error computing the stack trace.
    pub stack_trace_result: Option<StackTraceCreationResult<String>>,
    /// This may contain zero or more traces, depending on the (batch) request
    pub call_trace_arenas: Vec<CallTraceArena>,
}

impl From<String> for Response {
    fn from(value: String) -> Self {
        Response {
            data: Either::A(value),
            stack_trace_result: None,
            call_trace_arenas: Vec::new(),
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
        response: Result<ResponseWithCallTraces, ProviderErrorForChainSpec<Self>>,
    ) -> napi::Result<Response>;
}

impl<TimerT: Clone + TimeSinceEpoch> SyncNapiSpec<TimerT> for L1ChainSpec {
    const CHAIN_TYPE: &'static str = edr_chain_l1::CHAIN_TYPE;

    fn cast_response(
        mut response: Result<ResponseWithCallTraces, ProviderErrorForChainSpec<Self>>,
    ) -> napi::Result<Response> {
        let stack_trace_result =
            response.as_ref().err().and_then(|error| {
                if let edr_provider::ProviderError::TransactionFailed(failure) = error {
                    if matches!(
                        failure.failure.reason,
                        edr_provider::TransactionFailureReason::OutOfGas(_)
                    ) {
                        None
                    } else {
                        let result = failure.failure.stack_trace_result.clone().map_halt_reason(
                            |halt_reason| {
                                serde_json::to_string(&halt_reason)
                                    .expect("Failed to serialize halt reason")
                            },
                        );

                        Some(result)
                    }
                } else {
                    None
                }
            });

        // We can take the call trace arenas as they won't be used for anything else
        let call_trace_arenas = match &mut response {
            Ok(response) => std::mem::take(&mut response.call_trace_arenas),
            Err(edr_provider::ProviderError::TransactionFailed(failure)) => {
                std::mem::take(&mut failure.call_trace_arenas)
            }
            Err(_) => Vec::new(),
        };

        let response = jsonrpc::ResponseData::from(response.map(|response| response.result));

        marshal_response_data(response).map(|data| Response {
            data,
            stack_trace_result,
            call_trace_arenas,
        })
    }
}

impl<TimerT: Clone + TimeSinceEpoch> SyncNapiSpec<TimerT> for GenericChainSpec {
    const CHAIN_TYPE: &'static str = edr_generic::CHAIN_TYPE;

    fn cast_response(
        mut response: Result<ResponseWithCallTraces, ProviderErrorForChainSpec<Self>>,
    ) -> napi::Result<Response> {
        let stack_trace_result =
            response.as_ref().err().and_then(|error| {
                if let edr_provider::ProviderError::TransactionFailed(failure) = error {
                    if matches!(
                        failure.failure.reason,
                        edr_provider::TransactionFailureReason::OutOfGas(_)
                    ) {
                        None
                    } else {
                        let result = failure.failure.stack_trace_result.clone().map_halt_reason(
                            |halt_reason| {
                                serde_json::to_string(&halt_reason)
                                    .expect("Failed to serialize halt reason")
                            },
                        );

                        Some(result)
                    }
                } else {
                    None
                }
            });

        // We can take the traces as they won't be used for anything else
        let call_trace_arenas = match &mut response {
            Ok(response) => std::mem::take(&mut response.call_trace_arenas),
            Err(edr_provider::ProviderError::TransactionFailed(failure)) => {
                std::mem::take(&mut failure.call_trace_arenas)
            }
            Err(_) => Vec::new(),
        };

        let response = jsonrpc::ResponseData::from(response.map(|response| response.result));

        marshal_response_data(response).map(|data| Response {
            data,
            stack_trace_result,
            call_trace_arenas,
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
