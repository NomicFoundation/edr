use std::sync::Arc;

use edr_blockchain_api::BlockHashByNumber;
use edr_chain_spec_evm::{
    interpreter::{
        CallInputs, CallOutcome, CreateInputs, CreateOutcome, EthInterpreter, Interpreter,
    },
    ContextTrait, Inspector, JournalTrait,
};
use edr_coverage::{reporter::SyncOnCollectedCoverageCallback, CodeCoverageReporter};
use edr_database_components::{DatabaseComponents, WrapDatabaseRef};
use edr_inspector_bytecode::ExecutedBytecodeCollector;
use edr_primitives::{Address, Bytes, HashMap, HashSet};
use edr_receipt::log::ExecutionLog;
use edr_solidity::{
    config::IncludeTraces,
    contract_decoder::ContractDecoder,
    recorded_trace_arena::RecordedTraceArena,
    tracing::{SolidityTracingInspector, SolidityTracingInspectorArgs},
};
use edr_state_api::State;
use parking_lot::RwLock;
use revm_inspector::JournalExt;
use revm_inspectors::tracing::StackSnapshotType;

use crate::{call_override::Mocker, console_log::ConsoleLogCollector, SyncCallOverride};

/// Configuration for a [`EvmObserver`].
#[derive(Clone)]
pub struct EvmObserverConfig {
    pub call_override: Option<Arc<dyn SyncCallOverride>>,
    pub include_call_traces: IncludeTraces,
    pub contract_decoder: Arc<RwLock<ContractDecoder>>,
    pub on_collected_coverage_fn: Option<Box<dyn SyncOnCollectedCoverageCallback>>,
    pub record_memory: bool,
    pub record_stack: StackSnapshotType,
}

impl From<&EvmObserverConfig> for SolidityTracingInspectorArgs {
    fn from(config: &EvmObserverConfig) -> Self {
        Self {
            contract_decoder: config.contract_decoder.clone(),
            record_memory: config.record_memory,
            record_stack: config.record_stack,
        }
    }
}

/// An observer for the EVM that collects information about the execution by
/// directly inspecting the EVM.
///
/// The observer is stateless, without any awareness of when a transaction
/// starts or ends.
pub struct EvmObserver {
    bytecode_collector: ExecutedBytecodeCollector,
    code_coverage: Option<CodeCoverageReporter>,
    console_logger: ConsoleLogCollector,
    mocker: Mocker,
    tracing_inspector: SolidityTracingInspector,
}

#[derive(Debug, thiserror::Error)]
pub enum EvmObserverCollectionError {
    // TODO: This error should be caught when we originally parse the contract ABIs.
    /// An error occurred while ABI decoding the traces due to invalid input
    /// data.
    #[error(transparent)]
    AbiDecoding(serde_json::Error),
    /// An error occurred while invoking a `SyncOnCollectedCoverageCallback`.
    #[error(transparent)]
    OnCollectedCoverageCallback(Box<dyn std::error::Error + Send + Sync>),
}

#[derive(Debug)]
pub struct EvmObservedData {
    /// Mapping of contract address to executed bytecode
    pub address_to_executed_code: HashMap<Address, Bytes>,
    /// The trace collected during execution, including ABI-decoded
    /// information.
    pub execution_trace: RecordedTraceArena,
    /// Encoded `console.log` call inputs
    pub encoded_console_logs: Vec<Bytes>,
}

impl EvmObservedData {
    /// Returns the call traces, if `include` selects them for an execution
    /// with the given success.
    pub fn into_call_traces(
        self,
        include: IncludeTraces,
        is_success: bool,
    ) -> Option<RecordedTraceArena> {
        include
            .should_include(|| !is_success)
            .then_some(self.execution_trace)
    }
}

impl EvmObserver {
    /// Creates a new instance with the provided configuration.
    pub fn new(config: EvmObserverConfig) -> Self {
        // The tracing inspector must be created unconditionally: the
        // recorded traces are needed to generate stack traces for
        // transaction failures (see `get_stack_trace`) and to detect
        // internal out-of-gas errors during gas estimation (see
        // `has_internal_oog`). Whether traces are also included in
        // responses is decided separately, according to
        // `include_call_traces`.
        let tracing_inspector = SolidityTracingInspector::new((&config).into());

        let code_coverage = config
            .on_collected_coverage_fn
            .map(CodeCoverageReporter::new);

        Self {
            bytecode_collector: ExecutedBytecodeCollector::default(),
            code_coverage,
            console_logger: ConsoleLogCollector::default(),
            mocker: Mocker::new(config.call_override.clone()),
            tracing_inspector,
        }
    }

    /// Collects and reports the observed data of a single transaction.
    pub fn collect_and_report(
        self,
        precompile_addresses: &HashSet<Address>,
    ) -> Result<EvmObservedData, EvmObserverCollectionError> {
        let Self {
            bytecode_collector,
            code_coverage,
            console_logger,
            mocker: _mocker,
            tracing_inspector,
        } = self;

        if let Some(code_coverage) = code_coverage {
            code_coverage
                .collect_and_report()
                .map_err(EvmObserverCollectionError::OnCollectedCoverageCallback)?;
        }

        let address_to_executed_code = bytecode_collector.collect();
        let execution_trace = tracing_inspector
            .collect(&address_to_executed_code, precompile_addresses)
            .map_err(EvmObserverCollectionError::AbiDecoding)?;

        Ok(EvmObservedData {
            address_to_executed_code,
            execution_trace,
            encoded_console_logs: console_logger.into_encoded_messages(),
        })
    }
}

impl<
        BlockchainT: BlockHashByNumber<Error: 'static + std::error::Error + Send + Sync>,
        ContextT: ContextTrait<
            Journal: JournalExt
                         + JournalTrait<
                Database = WrapDatabaseRef<DatabaseComponents<BlockchainT, StateT>>,
            >,
        >,
        StateT: State<Error: 'static + std::error::Error + Send + Sync>,
    > Inspector<ContextT, EthInterpreter> for EvmObserver
{
    fn call(&mut self, context: &mut ContextT, inputs: &mut CallInputs) -> Option<CallOutcome> {
        if let Some(outcome) = self.bytecode_collector.call(context, inputs) {
            return Some(outcome);
        }

        self.console_logger.call(context, inputs);
        self.tracing_inspector.call(context, inputs);

        if let Some(outcome) = self.code_coverage.as_mut().and_then(|code_coverage| {
            Inspector::<_, EthInterpreter>::call(&mut code_coverage.collector, context, inputs)
        }) {
            // Inner inspector is short-circuiting the call. We should preserve its outcome.
            return Some(outcome);
        }
        self.mocker.call(context, inputs)
    }

    fn call_end(&mut self, context: &mut ContextT, inputs: &CallInputs, outcome: &mut CallOutcome) {
        self.tracing_inspector.call_end(context, inputs, outcome);
        if let Some(code_coverage) = self.code_coverage.as_mut() {
            Inspector::<_, EthInterpreter>::call_end(
                &mut code_coverage.collector,
                context,
                inputs,
                outcome,
            );
        }
    }

    fn create(
        &mut self,
        context: &mut ContextT,
        inputs: &mut CreateInputs,
    ) -> Option<CreateOutcome> {
        self.tracing_inspector.create(context, inputs)
    }

    fn create_end(
        &mut self,
        context: &mut ContextT,
        inputs: &CreateInputs,
        outcome: &mut CreateOutcome,
    ) {
        self.tracing_inspector.create_end(context, inputs, outcome);
        if let Some(code_coverage) = self.code_coverage.as_mut() {
            Inspector::<_, EthInterpreter>::create_end(
                &mut code_coverage.collector,
                context,
                inputs,
                outcome,
            );
        }
    }

    fn step(&mut self, interp: &mut Interpreter<EthInterpreter>, context: &mut ContextT) {
        self.tracing_inspector.step(interp, context);
    }

    fn log(&mut self, context: &mut ContextT, log: ExecutionLog) {
        self.tracing_inspector.log(context, log);
    }

    fn log_full(
        &mut self,
        interp: &mut Interpreter<EthInterpreter>,
        context: &mut ContextT,
        log: ExecutionLog,
    ) {
        self.tracing_inspector.log_full(interp, context, log);
    }
}
