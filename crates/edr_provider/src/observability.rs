use edr_eth::spec::HaltReasonTrait;
use edr_evm::{
    blockchain::BlockHash,
    inspector::Inspector,
    interpreter::{
        CallInputs, CallOutcome, CreateInputs, CreateOutcome, EOFCreateInputs, EthInterpreter,
        Interpreter,
    },
    journal::{JournalExt, JournalTrait},
    spec::ContextTrait,
    state::{DatabaseComponents, State, WrapDatabaseRef},
    trace::TraceCollector,
};

use crate::{console_log::ConsoleLogCollector, mock::Mocker};

pub struct RuntimeObserver<HaltReasonT: HaltReasonTrait> {
    pub console_logger: ConsoleLogCollector,
    pub mocker: Mocker,
    pub trace_collector: TraceCollector<HaltReasonT>,
}

impl<HaltReasonT: HaltReasonTrait> RuntimeObserver<HaltReasonT> {
    /// Creates a new instance with the provided mocker.
    /// If verbose is true, full stack and memory will be recorded for each
    /// step.
    pub fn with_mocker(mocker: Mocker, verbose: bool) -> Self {
        Self {
            console_logger: ConsoleLogCollector::default(),
            mocker,
            trace_collector: TraceCollector::new(verbose),
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

    fn eofcreate(
        &mut self,
        context: &mut ContextT,
        inputs: &mut EOFCreateInputs,
    ) -> Option<CreateOutcome> {
        self.trace_collector.eofcreate(context, inputs)
    }

    fn eofcreate_end(
        &mut self,
        context: &mut ContextT,
        inputs: &EOFCreateInputs,
        outcome: &mut CreateOutcome,
    ) {
        self.trace_collector.eofcreate_end(context, inputs, outcome);
    }

    fn step(&mut self, interp: &mut Interpreter<EthInterpreter>, context: &mut ContextT) {
        self.trace_collector.step(interp, context);
    }
}
