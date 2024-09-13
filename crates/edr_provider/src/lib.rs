mod config;
mod console_log;
mod data;
mod debug_mine;
mod debugger;
mod error;
mod filter;
mod interval;
mod logger;
mod mock;
mod pending;
mod provider;
/// Type for RPC requests.
pub mod requests;
mod snapshot;
/// Types for provider-related chain specification.
pub mod spec;
mod subscribe;
/// Utilities for testing
#[cfg(any(test, feature = "test-utils"))]
pub mod test_utils;
/// Types for temporal operations
pub mod time;

use core::fmt::Debug;

use edr_eth::HashSet;
// Re-export parts of `edr_evm`
pub use edr_evm::hardfork;
use edr_evm::{chain_spec::ChainSpec, trace::Trace};
use lazy_static::lazy_static;

pub use self::{
    config::*,
    data::{CallResult, ProviderData},
    debug_mine::DebugMineBlockResult,
    error::{EstimateGasFailure, ProviderError, TransactionFailure, TransactionFailureReason},
    logger::{Logger, NoopLogger, SyncLogger},
    mock::{CallOverrideResult, SyncCallOverride},
    provider::Provider,
    requests::{
        hardhat::rpc_types as hardhat_rpc_types, IntervalConfig as IntervalConfigRequest,
        InvalidRequestReason, MethodInvocation, ProviderRequest, Timestamp,
    },
    spec::{ProviderSpec, SyncProviderSpec},
    subscribe::*,
};

lazy_static! {
    pub static ref PRIVATE_RPC_METHODS: HashSet<&'static str> = {
        [
            "hardhat_getStackTraceFailuresCount",
            "hardhat_setLoggingEnabled",
        ]
        .into_iter()
        .collect()
    };
}

#[derive(Clone, Debug)]
pub struct ResponseWithTraces<ChainSpecT: edr_eth::chain_spec::EvmWiring> {
    pub result: serde_json::Value,
    pub traces: Vec<Trace<ChainSpecT>>,
}

fn to_json<T: serde::Serialize, ChainSpecT: ChainSpec<Hardfork: Debug>>(
    value: T,
) -> Result<ResponseWithTraces<ChainSpecT>, ProviderError<ChainSpecT>> {
    let response = serde_json::to_value(value).map_err(ProviderError::Serialization)?;

    Ok(ResponseWithTraces {
        result: response,
        traces: Vec::new(),
    })
}

fn to_json_with_trace<T: serde::Serialize, ChainSpecT: ChainSpec<Hardfork: Debug>>(
    value: (T, Trace<ChainSpecT>),
) -> Result<ResponseWithTraces<ChainSpecT>, ProviderError<ChainSpecT>> {
    let response = serde_json::to_value(value.0).map_err(ProviderError::Serialization)?;

    Ok(ResponseWithTraces {
        result: response,
        traces: vec![value.1],
    })
}

fn to_json_with_traces<T: serde::Serialize, ChainSpecT: ChainSpec<Hardfork: Debug>>(
    value: (T, Vec<Trace<ChainSpecT>>),
) -> Result<ResponseWithTraces<ChainSpecT>, ProviderError<ChainSpecT>> {
    let response = serde_json::to_value(value.0).map_err(ProviderError::Serialization)?;

    Ok(ResponseWithTraces {
        result: response,
        traces: value.1,
    })
}
