use core::fmt::Debug;
use std::sync::Arc;

use edr_coverage::{reporter::SyncOnCollectedCoverageCallback, CodeCoverageReporter};
use edr_eth::spec::HaltReasonTrait;
use edr_evm::{
    blockchain::BlockHash,
    inspector::Inspector,
    interpreter::{
        CallInputs, CallOutcome, CreateInputs, CreateOutcome, EthInterpreter, Interpreter,
    },
    journal::{JournalExt, JournalTrait},
    spec::ContextTrait,
    state::{DatabaseComponents, State, WrapDatabaseRef},
    trace::TraceCollector,
};

use crate::{console_log::ConsoleLogCollector, mock::Mocker, SyncCallOverride};

/// Configuration for a [`RuntimeObserver`].
#[derive(Clone, Default)]
pub struct Config {
    pub call_override: Option<Arc<dyn SyncCallOverride>>,
    pub on_collected_coverage_fn: Option<Box<dyn SyncOnCollectedCoverageCallback>>,
    pub verbose_raw_tracing: bool,
}

impl Debug for Config {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Config")
            .field("call_override", &self.call_override.is_some())
            .field(
                "on_collected_coverage_fn",
                &self.on_collected_coverage_fn.is_some(),
            )
            .field("verbose_raw_traces", &self.verbose_raw_tracing)
            .finish()
    }
}

/// An observer for the EVM runtime that collects information about the
/// execution.
pub struct RuntimeObserver<HaltReasonT: HaltReasonTrait> {
    pub code_coverage: Option<CodeCoverageReporter>,
    pub console_logger: ConsoleLogCollector,
    pub mocker: Mocker,
    pub trace_collector: TraceCollector<HaltReasonT>,
}

impl<HaltReasonT: HaltReasonTrait> RuntimeObserver<HaltReasonT> {
    /// Creates a new instance with the provided configuration.
    pub fn new(config: Config) -> Self {
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
    > Inspector<ContextT, EthInterpreter> for RuntimeObserver<HaltReasonT>
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
