use core::fmt::Debug;
use std::sync::Arc;

use edr_blockchain_api::BlockHash;
use edr_coverage::{reporter::SyncOnCollectedCoverageCallback, CodeCoverageReporter};
use edr_database_components::DatabaseComponents;
use edr_evm::{
    inspector::Inspector,
    interpreter::{
        CallInputs, CallOutcome, CreateInputs, CreateOutcome, EthInterpreter, Interpreter,
    },
    journal::{JournalExt, JournalTrait},
    spec::ContextTrait,
    state::WrapDatabaseRef,
    trace::TraceCollector,
};
use edr_evm_spec::HaltReasonTrait;
use edr_state_api::State;

use crate::{
    console_log::ConsoleLogCollector, gas_reports::SyncOnCollectedGasReportCallback, mock::Mocker,
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
    pub on_collected_coverage_fn: Option<Box<dyn SyncOnCollectedCoverageCallback>>,
    pub on_collected_gas_report_fn: Option<Box<dyn SyncOnCollectedGasReportCallback>>,
    pub verbose_raw_tracing: bool,
}

impl Debug for ObservabilityConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Config")
            .field("call_override", &self.call_override.is_some())
            .field(
                "on_collected_coverage_fn",
                &self.on_collected_coverage_fn.is_some(),
            )
            .field(
                "on_collected_gas_report_fn",
                &self.on_collected_gas_report_fn.is_some(),
            )
            .field("verbose_raw_traces", &self.verbose_raw_tracing)
            .finish()
    }
}

/// Configuration for a [`EvmObserver`].
pub struct EvmObserverConfig {
    pub call_override: Option<Arc<dyn SyncCallOverride>>,
    pub on_collected_coverage_fn: Option<Box<dyn SyncOnCollectedCoverageCallback>>,
    pub verbose_raw_tracing: bool,
}

impl From<&ObservabilityConfig> for EvmObserverConfig {
    fn from(value: &ObservabilityConfig) -> Self {
        Self {
            call_override: value.call_override.clone(),
            on_collected_coverage_fn: value.on_collected_coverage_fn.clone(),
            verbose_raw_tracing: value.verbose_raw_tracing,
        }
    }
}

/// An observer for the EVM that collects information about the execution by
/// directly inspecting the EVM.
pub struct EvmObserver<HaltReasonT: HaltReasonTrait> {
    pub code_coverage: Option<CodeCoverageReporter>,
    pub console_logger: ConsoleLogCollector,
    pub mocker: Mocker,
    pub trace_collector: TraceCollector<HaltReasonT>,
}

impl<HaltReasonT: HaltReasonTrait> EvmObserver<HaltReasonT> {
    /// Creates a new instance with the provided configuration.
    pub fn new(config: EvmObserverConfig) -> Self {
        let code_coverage = config
            .on_collected_coverage_fn
            .map(CodeCoverageReporter::new);

        Self {
            code_coverage,
            console_logger: ConsoleLogCollector::default(),
            mocker: Mocker::new(config.call_override.clone()),
            trace_collector: TraceCollector::new(config.verbose_raw_tracing),
        }
    }
}

impl<
        BlockchainT: BlockHash<Error: std::error::Error>,
        ContextT: ContextTrait<
            Journal: JournalExt
                         + JournalTrait<
                Database = WrapDatabaseRef<DatabaseComponents<BlockchainT, StateT>>,
            >,
        >,
        HaltReasonT: HaltReasonTrait,
        StateT: State<Error: std::error::Error>,
    > Inspector<ContextT, EthInterpreter> for EvmObserver<HaltReasonT>
{
    fn call(&mut self, context: &mut ContextT, inputs: &mut CallInputs) -> Option<CallOutcome> {
        self.console_logger.call(context, inputs);
        if let Some(code_coverage) = &mut self.code_coverage {
            Inspector::<_, EthInterpreter>::call(&mut code_coverage.collector, context, inputs);
        }
        self.trace_collector.call(context, inputs);
        self.mocker.call(context, inputs)
    }

    fn call_end(&mut self, context: &mut ContextT, inputs: &CallInputs, outcome: &mut CallOutcome) {
        self.trace_collector.call_end(context, inputs, outcome);
    }

    fn create(
        &mut self,
        context: &mut ContextT,
        inputs: &mut CreateInputs,
    ) -> Option<CreateOutcome> {
        self.trace_collector.create(context, inputs)
    }

    fn create_end(
        &mut self,
        context: &mut ContextT,
        inputs: &CreateInputs,
        outcome: &mut CreateOutcome,
    ) {
        self.trace_collector.create_end(context, inputs, outcome);
    }

    fn step(&mut self, interp: &mut Interpreter<EthInterpreter>, context: &mut ContextT) {
        self.trace_collector.step(interp, context);
    }
}
