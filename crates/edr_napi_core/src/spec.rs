use core::fmt::Debug;

use edr_chain_l1::L1ChainSpec;
use edr_chain_spec::{HaltReasonTrait, TransactionValidation};
use edr_generic::GenericChainSpec;
use edr_provider::{
    time::TimeSinceEpoch, ProviderError, ProviderErrorForChainSpec, ResponseWithCallTraces,
    SyncProviderSpec,
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

/// Casts a [`Result`] received from a provider into a [`Response`] that can be
/// returned to N-API, taking into account the possibility of large responses
/// and the presence of stack traces in case of transaction failures.
pub fn cast_provider_result_to_response<
    FetchReceiptErrorT: std::error::Error,
    GenesisBlockCreationErrorT: std::error::Error,
    HaltReasonT: HaltReasonTrait + serde::Serialize,
    HardforkT: Debug,
    TransactionValidationErrorT: std::error::Error,
>(
    mut response: Result<
        ResponseWithCallTraces,
        ProviderError<
            FetchReceiptErrorT,
            GenesisBlockCreationErrorT,
            HaltReasonT,
            HardforkT,
            TransactionValidationErrorT,
        >,
    >,
) -> napi::Result<Response> {
    let stack_trace_result = response.as_ref().err().and_then(|error| {
        if let edr_provider::ProviderError::TransactionFailed(failure) = error {
            if matches!(
                failure.failure.reason,
                edr_provider::TransactionFailureReason::OutOfGas(_)
            ) {
                None
            } else {
                let result =
                    failure
                        .failure
                        .stack_trace_result
                        .clone()
                        .map_halt_reason(|halt_reason| {
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

impl<TimerT: Clone + TimeSinceEpoch> SyncNapiSpec<TimerT> for L1ChainSpec {
    const CHAIN_TYPE: &'static str = edr_chain_l1::CHAIN_TYPE;

    fn cast_response(
        response: Result<ResponseWithCallTraces, ProviderErrorForChainSpec<Self>>,
    ) -> napi::Result<Response> {
        cast_provider_result_to_response(response)
    }
}

impl<TimerT: Clone + TimeSinceEpoch> SyncNapiSpec<TimerT> for GenericChainSpec {
    const CHAIN_TYPE: &'static str = edr_generic::CHAIN_TYPE;

    fn cast_response(
        response: Result<ResponseWithCallTraces, ProviderErrorForChainSpec<Self>>,
    ) -> napi::Result<Response> {
        cast_provider_result_to_response(response)
    }
}

/// Marshals a JSON-RPC response data into a `ResponseData`, taking into account
/// large responses.
pub fn marshal_response_data(
    response: jsonrpc::ResponseData<Box<serde_json::value::RawValue>>,
) -> napi::Result<ResponseData> {
    // We experimentally determined that 500_000_000 was the maximum string length
    // that can be returned without causing the error:
    //
    // > Failed to convert rust `String` into napi `string`
    //
    // To be safe, we're limiting string lengths to half of that.
    const MAX_STRING_LENGTH: usize = 250_000_000;

    // A success envelope's length is known before it is built, so an oversized
    // response never has to be serialized only to be discarded.
    if let jsonrpc::ResponseData::Success { result } = &response
        && envelope_len(result) > MAX_STRING_LENGTH
    {
        return serde_json::to_value(response)
            .map(Either::B)
            .map_err(|error| napi::Error::new(Status::GenericFailure, error.to_string()));
    }

    let json = serialize_response(&response)
        .map_err(|error| napi::Error::new(Status::GenericFailure, error.to_string()))?;

    if json.len() <= MAX_STRING_LENGTH {
        Ok(Either::A(json))
    } else {
        serde_json::to_value(response)
            .map(Either::B)
            .map_err(|error| napi::Error::new(Status::GenericFailure, error.to_string()))
    }
}

/// Returns the length of the response envelope around an already-serialized
/// result.
fn envelope_len(result: &serde_json::value::RawValue) -> usize {
    // `{"result":` and `}`
    const OVERHEAD: usize = 11;

    result.get().len() + OVERHEAD
}

/// Serializes a JSON-RPC response into a buffer that never has to grow.
///
/// `serde_json::to_string` starts at 128 bytes and doubles, so it copies a
/// large result several times over.
fn serialize_response(
    response: &jsonrpc::ResponseData<Box<serde_json::value::RawValue>>,
) -> Result<String, serde_json::Error> {
    /// What `serde_json::to_string` would start from, for the envelope whose
    /// length is not known up front.
    const ERROR_CAPACITY: usize = 128;

    let capacity = match response {
        jsonrpc::ResponseData::Success { result } => envelope_len(result),
        jsonrpc::ResponseData::Error { .. } => ERROR_CAPACITY,
    };

    let mut buffer = Vec::with_capacity(capacity);
    serde_json::to_writer(&mut buffer, response)?;

    // SAFETY: `serde_json` only emits UTF-8.
    Ok(unsafe { String::from_utf8_unchecked(buffer) })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn raw_value(json: &str) -> Box<serde_json::value::RawValue> {
        serde_json::value::RawValue::from_string(json.to_string()).expect("the JSON is valid")
    }

    fn assert_matches_serde(response: jsonrpc::ResponseData<Box<serde_json::value::RawValue>>) {
        let expected = serde_json::to_string(&response).expect("serialization succeeds");

        let json = match marshal_response_data(response).expect("marshalling succeeds") {
            Either::A(json) => json,
            Either::B(_) => panic!("the response is small enough to be a string"),
        };

        assert_eq!(json, expected);
    }

    #[test]
    fn marshal_response_data_matches_serde_for_success() {
        let results = [
            "null",
            "true",
            "\"0x1\"",
            "[]",
            "[\"0x1\",{\"a\":[1,2,3]}]",
            "{\"blockNumber\":\"0x1\",\"logs\":[]}",
            "115792089237316195423570985008687907853269984665640564039457584007913129639935",
        ];

        for result in results {
            assert_matches_serde(jsonrpc::ResponseData::Success {
                result: raw_value(result),
            });
        }
    }

    #[test]
    fn marshal_response_data_matches_serde_for_errors() {
        assert_matches_serde(jsonrpc::ResponseData::new_error(
            -32000,
            "boom",
            Some(serde_json::json!({ "method": "eth_call" })),
        ));
    }
}
