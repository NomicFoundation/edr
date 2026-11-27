use core::fmt::Debug;
use std::sync::Arc;

use edr_coverage::reporter::SyncOnCollectedCoverageCallback;
use edr_gas_report::SyncOnCollectedGasReportCallback;
pub use edr_inspection::{
    EvmObservedData, EvmObserver, EvmObserverCollectionError, EvmObserverConfig,
};
use edr_solidity::{config::IncludeTraces, contract_decoder::ContractDecoder};
use parking_lot::RwLock;
pub use revm_inspectors::tracing::StackSnapshotType;

use crate::{
    error::{JsonRpcError, INTERNAL_ERROR},
    SyncCallOverride,
};

/// Convenience type alias for [`ObservabilityConfig`].
///
/// This allows usage like `edr_provider::observability::Config`.
pub type Config = ObservabilityConfig;

/// Configuration for collecting information about executed transactions.
///
/// This can happen at multiple levels:
/// - **EVM-level**: Using a [`EvmObserver`] to inspect the EVM execution.
/// - **Provider-level**: Using finalised execution results.
#[derive(Clone, Default)]
pub struct ObservabilityConfig {
    pub call_override: Option<Arc<dyn SyncCallOverride>>,
    pub include_call_traces: IncludeTraces,
    pub on_collected_coverage_fn: Option<Box<dyn SyncOnCollectedCoverageCallback>>,
    pub on_collected_gas_report_fn: Option<Box<dyn SyncOnCollectedGasReportCallback>>,
    /// How much of the stack each recorded step carries.
    pub record_stack: StackSnapshotType,
    /// Records the full stack and memory while enabled. `record_stack` keeps
    /// its configured value meanwhile.
    pub verbose_raw_tracing: bool,
}

impl ObservabilityConfig {
    /// Whether recorded steps carry a memory snapshot.
    pub fn records_memory(&self) -> bool {
        self.verbose_raw_tracing
    }

    /// How much of the stack recorded steps carry.
    pub fn recorded_stack(&self) -> StackSnapshotType {
        recorded_stack(self.record_stack, self.verbose_raw_tracing)
    }
}

/// How much of the stack recorded steps carry, given the configured amount and
/// whether verbose raw tracing is enabled.
pub const fn recorded_stack(
    record_stack: StackSnapshotType,
    verbose_raw_tracing: bool,
) -> StackSnapshotType {
    if verbose_raw_tracing {
        StackSnapshotType::Full
    } else {
        record_stack
    }
}

impl Debug for ObservabilityConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Config")
            .field("call_override", &self.call_override.is_some())
            .field("include_call_traces", &self.include_call_traces)
            .field(
                "on_collected_coverage_fn",
                &self.on_collected_coverage_fn.is_some(),
            )
            .field(
                "on_collected_gas_report_fn",
                &self.on_collected_gas_report_fn.is_some(),
            )
            .field("record_stack", &self.record_stack)
            .field("verbose_raw_tracing", &self.verbose_raw_tracing)
            .finish()
    }
}

/// Creates the configuration of an [`EvmObserver`] from the provided
/// [`ObservabilityConfig`].
pub fn evm_observer_config(
    config: &ObservabilityConfig,
    contract_decoder: Arc<RwLock<ContractDecoder>>,
) -> EvmObserverConfig {
    EvmObserverConfig {
        call_override: config.call_override.clone(),
        contract_decoder,
        include_call_traces: config.include_call_traces,
        on_collected_coverage_fn: config.on_collected_coverage_fn.clone(),
        record_memory: config.records_memory(),
        record_stack: config.recorded_stack(),
    }
}

impl JsonRpcError for EvmObserverCollectionError {
    fn error_code(&self) -> i16 {
        match self {
            EvmObserverCollectionError::AbiDecoding(_)
            | EvmObserverCollectionError::OnCollectedCoverageCallback(_) => INTERNAL_ERROR,
        }
    }
}
