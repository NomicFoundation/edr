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
/// Type for RPC requests.
pub mod requests;
mod sequential;
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
use std::sync::Arc;

use edr_eth::{
    result::InvalidTransaction,
    transaction::{IsEip155, IsEip4844, TransactionMut, TransactionType, TransactionValidation},
    HashSet,
};
// Re-export parts of `edr_evm`
pub use edr_evm::hardfork;
use edr_evm::{blockchain::BlockchainError, chain_spec::ChainSpec, trace::Trace};
use edr_rpc_eth::jsonrpc::Response;
use lazy_static::lazy_static;
use logger::SyncLogger;
use mock::SyncCallOverride;
use parking_lot::Mutex;
use requests::{eth::handle_set_interval_mining, hardhat::rpc_types::ResetProviderConfig};
use time::{CurrentTime, TimeSinceEpoch};
use tokio::{runtime, sync::Mutex as AsyncMutex, task};

pub use self::{
    config::*,
    data::{CallResult, ProviderData},
    debug_mine::DebugMineBlockResult,
    error::{EstimateGasFailure, ProviderError, TransactionFailure, TransactionFailureReason},
    logger::{Logger, NoopLogger},
    mock::CallOverrideResult,
    requests::{
        hardhat::rpc_types as hardhat_rpc_types, IntervalConfig as IntervalConfigRequest,
        InvalidRequestReason, MethodInvocation, ProviderRequest, Timestamp,
    },
    sequential::Sequential,
    spec::{ProviderSpec, SyncProviderSpec},
    subscribe::*,
};
use self::{
    data::CreationError,
    interval::IntervalMiner,
    requests::{debug, eth, hardhat},
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

pub trait Provider {
    type Error;

    /// Blocking method to handle a request.
    fn handle_request(
        &self,
        request: serde_json::Value,
    ) -> Result<Response<serde_json::Value>, Self::Error>;

    fn set_call_override_callback(&self, call_override_callback: CallOverrideCallback);

    fn set_verbose_tracing(&self, enabled: bool);
}

pub trait SyncProvider: Provider + Send + Sync {}

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
