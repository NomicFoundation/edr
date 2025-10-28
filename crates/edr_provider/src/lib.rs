/// Types for configuring the provider.
pub mod config;
mod console_log;
mod data;
mod debug_mine;
mod debug_trace;
mod error;
mod filter;
pub mod gas_reports;
mod interval;
mod logger;
mod mock;
/// Types for runtime observability.
pub mod observability;
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
mod utils;

use core::fmt::Debug;

use edr_chain_spec::{ChainSpec, HaltReasonTrait};
use edr_primitives::HashSet;
use edr_runtime::trace::Trace;
use lazy_static::lazy_static;

pub use self::{
    config::{
        AccountOverride, Fork as ForkConfig, Interval as IntervalConfig, MemPool as MemPoolConfig,
        Mining as MiningConfig, Provider as ProviderConfig,
    },
    data::{CallResult, ProviderData},
    debug_mine::{DebugMineBlockResult, DebugMineBlockResultForChainSpec},
    debug_trace::DebugTraceError,
    error::{
        EstimateGasFailure, ProviderError, ProviderErrorForChainSpec, TransactionFailure,
        TransactionFailureReason,
    },
    logger::{Logger, NoopLogger, SyncLogger},
    mock::{CallOverrideResult, SyncCallOverride},
    provider::Provider,
    requests::{
        eth::calculate_eip1559_fee_parameters, hardhat::rpc_types as hardhat_rpc_types,
        IntervalConfig as IntervalConfigRequest, InvalidRequestReason, MethodInvocation,
        ProviderRequest, Timestamp,
    },
    spec::{ProviderSpec, SyncProviderSpec},
    subscribe::*,
};
use crate::time::TimeSinceEpoch;

lazy_static! {
    pub static ref PRIVATE_RPC_METHODS: HashSet<&'static str> =
        ["hardhat_setLoggingEnabled",].into_iter().collect();
}

pub type ProviderResultWithTraces<T, ChainSpecT> = Result<
    (T, Vec<Trace<<ChainSpecT as ChainSpec>::HaltReason>>),
    ProviderErrorForChainSpec<ChainSpecT>,
>;

#[derive(Clone, Debug)]
pub struct ResponseWithTraces<HaltReasonT: HaltReasonTrait> {
    pub result: serde_json::Value,
    pub traces: Vec<Trace<HaltReasonT>>,
}

fn to_json<
    T: serde::Serialize,
    ChainSpecT: ProviderSpec<TimerT>,
    TimerT: Clone + TimeSinceEpoch,
>(
    value: T,
) -> Result<ResponseWithTraces<ChainSpecT::HaltReason>, ProviderErrorForChainSpec<ChainSpecT>> {
    let response = serde_json::to_value(value).map_err(ProviderError::Serialization)?;

    Ok(ResponseWithTraces {
        result: response,
        traces: Vec::new(),
    })
}

fn to_json_with_trace<
    T: serde::Serialize,
    ChainSpecT: ProviderSpec<TimerT>,
    TimerT: Clone + TimeSinceEpoch,
>(
    value: (T, Trace<ChainSpecT::HaltReason>),
) -> Result<ResponseWithTraces<ChainSpecT::HaltReason>, ProviderErrorForChainSpec<ChainSpecT>> {
    let response = serde_json::to_value(value.0).map_err(ProviderError::Serialization)?;

    Ok(ResponseWithTraces {
        result: response,
        traces: vec![value.1],
    })
}

fn to_json_with_traces<
    T: serde::Serialize,
    ChainSpecT: ProviderSpec<TimerT>,
    TimerT: Clone + TimeSinceEpoch,
>(
    value: (T, Vec<Trace<ChainSpecT::HaltReason>>),
) -> Result<ResponseWithTraces<ChainSpecT::HaltReason>, ProviderErrorForChainSpec<ChainSpecT>> {
    let response = serde_json::to_value(value.0).map_err(ProviderError::Serialization)?;

    Ok(ResponseWithTraces {
        result: response,
        traces: value.1,
    })
}
