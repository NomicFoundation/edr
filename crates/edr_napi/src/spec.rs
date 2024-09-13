use edr_eth::{
    chain_spec::L1ChainSpec,
    result::InvalidTransaction,
    transaction::{IsEip155, IsEip4844, TransactionMut, TransactionType, TransactionValidation},
};
use edr_generic::GenericChainSpec;
use edr_provider::{time::CurrentTime, ProviderError, ResponseWithTraces, SyncProviderSpec};
use edr_rpc_client::jsonrpc;
use napi::{Either, Status};
use napi_derive::napi;

use crate::trace::RawTrace;

#[napi]
pub struct Response {
    // N-API is known to be slow when marshalling `serde_json::Value`s, so we try to return a
    // `String`. If the object is too large to be represented as a `String`, we return a `Buffer`
    // instead.
    data: Either<String, serde_json::Value>,
    /// When a transaction fails to execute, the provider returns a trace of the
    /// transaction.
    ///
    /// Only present for L1 Ethereum chains.
    solidity_trace: Option<RawTrace>,
    /// This may contain zero or more traces, depending on the (batch) request
    ///
    /// Always empty for non-L1 Ethereum chains.
    traces: Vec<RawTrace>,
}

#[napi]
impl Response {
    #[doc = "Returns the response data as a JSON string or a JSON object."]
    #[napi(getter)]
    pub fn data(&self) -> Either<String, serde_json::Value> {
        self.data.clone()
    }

    #[doc = "Returns the Solidity trace of the transaction that failed to execute, if any."]
    #[napi(getter)]
    pub fn solidity_trace(&self) -> Option<RawTrace> {
        self.solidity_trace.clone()
    }

    #[doc = "Returns the raw traces of executed contracts. This maybe contain zero or more traces."]
    #[napi(getter)]
    pub fn traces(&self) -> Vec<RawTrace> {
        self.traces.clone()
    }
}

impl From<String> for Response {
    fn from(value: String) -> Self {
        Response {
            solidity_trace: None,
            data: Either::A(value),
            traces: Vec::new(),
        }
    }
}

/// Trait for a defining a chain's associated type in the N-API.
pub trait SyncNapiSpec:
    SyncProviderSpec<
    CurrentTime,
    Block: Clone + Default,
    PooledTransaction: IsEip155,
    Transaction: Default
                     + TransactionMut
                     + TransactionType<Type: IsEip4844>
                     + TransactionValidation<ValidationError: From<InvalidTransaction> + PartialEq>,
>
{
    /// The string type identifier of the chain.
    const CHAIN_TYPE: &'static str;

    /// Casts a response with traces into a `Response`.
    ///
    /// This is implemented as an associated function to avoid problems when
    /// implementing type conversions for third-party types.
    fn cast_response(
        response: Result<ResponseWithTraces<Self>, ProviderError<Self>>,
    ) -> napi::Result<Response>;
}

impl SyncNapiSpec for L1ChainSpec {
    const CHAIN_TYPE: &'static str = "L1";

    fn cast_response(
        response: Result<ResponseWithTraces<Self>, ProviderError<Self>>,
    ) -> napi::Result<Response> {
        let response = jsonrpc::ResponseData::from(response.map(|response| response.result));

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
            .map(|data| Response {
                solidity_trace: None,
                data,
                traces: Vec::new(),
            })
    }
}

impl SyncNapiSpec for GenericChainSpec {
    const CHAIN_TYPE: &'static str = "generic";

    fn cast_response(
        mut response: Result<ResponseWithTraces<Self>, ProviderError<Self>>,
    ) -> napi::Result<Response> {
        // We can take the solidity trace as it won't be used for anything else
        let solidity_trace = response.as_mut().err().and_then(|error| {
            if let edr_provider::ProviderError::TransactionFailed(failure) = error {
                if matches!(
                    failure.failure.reason,
                    edr_provider::TransactionFailureReason::OutOfGas(_)
                ) {
                    None
                } else {
                    Some(RawTrace::from(std::mem::take(
                        &mut failure.failure.solidity_trace,
                    )))
                }
            } else {
                None
            }
        });

        // We can take the traces as they won't be used for anything else
        let traces = match &mut response {
            Ok(response) => std::mem::take(&mut response.traces),
            Err(edr_provider::ProviderError::TransactionFailed(failure)) => {
                std::mem::take(&mut failure.traces)
            }
            Err(_) => Vec::new(),
        };

        let response = jsonrpc::ResponseData::from(response.map(|response| response.result));

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
            .map(|data| Response {
                solidity_trace,
                data,
                traces: traces.into_iter().map(RawTrace::from).collect(),
            })
    }
}
