use core::fmt::Debug;
use std::sync::Arc;

use edr_block_builder_api::WrapDatabaseRef;
use edr_block_miner::FlushInspectorData;
use edr_blockchain_api::BlockHashByNumber;
use edr_chain_spec_evm::{
    interpreter::{
        CallInputs, CallOutcome, CreateInputs, CreateOutcome, EthInterpreter, Interpreter,
    },
    ContextTrait, Inspector, JournalTrait,
};
use edr_coverage::{reporter::SyncOnCollectedCoverageCallback, CodeCoverageReporter};
use edr_database_components::DatabaseComponents;
use edr_evm::{ExecutionResultAndStateWithMetadata, ExecutionResultWithMetadata};
use edr_gas_report::SyncOnCollectedGasReportCallback;
use edr_inspector_bytecode::ExecutedBytecodeCollector;
use edr_primitives::{Address, Bytes, HashMap, HashSet};
use edr_receipt::ExecutionResult;
use edr_solidity::{
    config::IncludeTraces, contract_decoder::ContractDecoder, tracing::SolidityTracingInspector,
};
use edr_state_api::State;
use foundry_evm_traces::CallTraceArena;
use parking_lot::RwLock;
use revm_inspector::JournalExt;
use revm_inspectors::tracing::{TracingInspector, TracingInspectorConfig};

use crate::{
    console_log::ConsoleLogCollector,
    error::{JsonRpcError, INTERNAL_ERROR},
    mock::Mocker,
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
    pub verbose_raw_tracing: bool,
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
            .field("verbose_raw_traces", &self.verbose_raw_tracing)
            .finish()
    }
}

/// Configuration for a [`EvmObserver`].
#[derive(Clone)]
pub struct EvmObserverConfig {
    pub call_override: Option<Arc<dyn SyncCallOverride>>,
    pub include_call_traces: IncludeTraces,
    pub contract_decoder: Arc<RwLock<ContractDecoder>>,
    pub on_collected_coverage_fn: Option<Box<dyn SyncOnCollectedCoverageCallback>>,
    pub verbose_raw_tracing: bool,
}

impl EvmObserverConfig {
    /// Creates a new instance from the provided [`ObservabilityConfig`].
    pub fn new(
        config: &ObservabilityConfig,
        contract_decoder: Arc<RwLock<ContractDecoder>>,
    ) -> Self {
        Self {
            call_override: config.call_override.clone(),
            contract_decoder,
            include_call_traces: config.include_call_traces,
            on_collected_coverage_fn: config.on_collected_coverage_fn.clone(),
            verbose_raw_tracing: config.verbose_raw_tracing,
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

impl JsonRpcError for EvmObserverCollectionError {
    fn error_code(&self) -> i16 {
        match self {
            EvmObserverCollectionError::AbiDecoding(_)
            | EvmObserverCollectionError::OnCollectedCoverageCallback(_) => INTERNAL_ERROR,
        }
    }
}

#[derive(Debug)]
pub struct EvmObservedData {
    /// Mapping of contract address to executed bytecode
    pub address_to_executed_code: HashMap<Address, Bytes>,
    /// The call trace arena collected during execution, including ABI-decoded
    /// information.
    pub call_trace_arena: CallTraceArena,
    /// Encoded `console.log` call inputs
    pub encoded_console_logs: Vec<Bytes>,
}

impl EvmObservedData {
    pub(crate) fn into_call_traces(
        self,
        include: IncludeTraces,
        is_success: bool,
    ) -> Option<CallTraceArena> {
        include
            .should_include(|| !is_success)
            .then_some(self.call_trace_arena)
    }
}

impl EvmObserver {
    /// Creates a new instance with the provided configuration.
    pub fn new(config: EvmObserverConfig) -> Self {
        let code_coverage = config
            .on_collected_coverage_fn
            .map(CodeCoverageReporter::new);

        let tracing_config = if config.verbose_raw_tracing {
            TracingInspectorConfig::all()
        } else {
            TracingInspectorConfig::default_parity().set_steps(true)
        };

        Self {
            bytecode_collector: ExecutedBytecodeCollector::default(),
            code_coverage,
            console_logger: ConsoleLogCollector::default(),
            mocker: Mocker::new(config.call_override.clone()),
            // The tracing inspector must be created unconditionally: the
            // recorded traces are needed to generate stack traces for
            // transaction failures (see `get_stack_trace`) and to detect
            // internal out-of-gas errors during gas estimation (see
            // `has_internal_oog`). Whether traces are also included in
            // responses is decided separately, according to
            // `include_call_traces`.
            tracing_inspector: SolidityTracingInspector::new(
                TracingInspector::new(tracing_config),
                config.contract_decoder,
            ),
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
        let call_trace_arena = tracing_inspector
            .collect(&address_to_executed_code, precompile_addresses)
            .map_err(EvmObserverCollectionError::AbiDecoding)?;

        Ok(EvmObservedData {
            address_to_executed_code,
            call_trace_arena,
            encoded_console_logs: console_logger.into_encoded_messages(),
        })
    }

    pub fn flush_and_report(
        &mut self,
        precompile_addresses: &HashSet<Address>,
    ) -> Result<EvmObservedData, EvmObserverCollectionError> {
        let Self {
            bytecode_collector,
            code_coverage,
            console_logger,
            mocker: _mocker,
            tracing_inspector,
        } = self;

        let address_to_executed_code = bytecode_collector.take();

        if let Some(code_coverage) = code_coverage {
            code_coverage
                .flush_and_report()
                .map_err(EvmObserverCollectionError::OnCollectedCoverageCallback)?;
        }

        let call_trace_arena = tracing_inspector
            .take(&address_to_executed_code, precompile_addresses)
            .map_err(EvmObserverCollectionError::AbiDecoding)?;

        let encoded_console_logs = console_logger.take_encoded_messages();

        Ok(EvmObservedData {
            address_to_executed_code,
            call_trace_arena,
            encoded_console_logs,
        })
    }
}

impl FlushInspectorData for EvmObserver {
    type Output = EvmObservedData;

    type Error = EvmObserverCollectionError;

    fn flush_inspector_data(
        &mut self,
        precompile_addresses: &HashSet<Address>,
    ) -> Result<Self::Output, Self::Error> {
        self.flush_and_report(precompile_addresses)
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
}

/// Abstracts over [`ExecutionResultWithMetadata`] and
/// [`ExecutionResultAndStateWithMetadata`] so that [`observe_execution`] can be
/// generic over both without duplicating the observer lifecycle.
pub(crate) trait WithExecutionResult {
    type HaltReason;
    fn precompile_addresses(&self) -> &HashSet<Address>;
    fn result(&self) -> &ExecutionResult<Self::HaltReason>;
}

impl<HaltReasonT> WithExecutionResult for ExecutionResultWithMetadata<HaltReasonT> {
    type HaltReason = HaltReasonT;
    fn precompile_addresses(&self) -> &HashSet<Address> {
        &self.precompile_addresses
    }
    fn result(&self) -> &ExecutionResult<HaltReasonT> {
        &self.result
    }
}

impl<HaltReasonT> WithExecutionResult for ExecutionResultAndStateWithMetadata<HaltReasonT> {
    type HaltReason = HaltReasonT;
    fn precompile_addresses(&self) -> &HashSet<Address> {
        &self.precompile_addresses
    }
    fn result(&self) -> &ExecutionResult<HaltReasonT> {
        &self.result
    }
}
pub(crate) struct ObservedExecution<ExecutionResultT> {
    pub evm_observed_data: EvmObservedData,
    pub execution_result: ExecutionResultT,
    include_call_traces: IncludeTraces,
}

impl<ExecutionResultT: WithExecutionResult> ObservedExecution<ExecutionResultT> {
    pub fn result(&self) -> &ExecutionResult<ExecutionResultT::HaltReason> {
        self.execution_result.result()
    }

    /// Consumes the observed execution, returning the execution result and the
    /// call traces filtered by the observability configuration, using the
    /// EVM-level success of the execution as the success criteria.
    pub fn into_result_and_filtered_traces(self) -> (ExecutionResultT, Option<CallTraceArena>) {
        let should_include_traces = self
            .include_call_traces
            .should_include(|| !self.execution_result.result().is_success());

        let execution_result = self.execution_result;
        let call_trace_arena =
            should_include_traces.then_some(self.evm_observed_data.call_trace_arena);

        (execution_result, call_trace_arena)
    }
}

pub(crate) fn observe_execution<
    ExecutionResultT: WithExecutionResult,
    ErrorT: From<EvmObserverCollectionError>,
    ExecutionFn: FnOnce(&mut EvmObserver) -> Result<ExecutionResultT, ErrorT>,
>(
    observer_config: &EvmObserverConfig,
    observable_execution: ExecutionFn,
) -> Result<ObservedExecution<ExecutionResultT>, ErrorT> {
    let include_call_traces = observer_config.include_call_traces;
    let mut observer = EvmObserver::new(observer_config.clone());

    let execution_result = observable_execution(&mut observer)?;

    let evm_observed_data = observer.collect_and_report(execution_result.precompile_addresses())?;

    Ok(ObservedExecution {
        evm_observed_data,
        execution_result,
        include_call_traces,
    })
}
