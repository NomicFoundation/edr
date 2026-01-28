use core::fmt::Debug;
use std::sync::Arc;

use edr_block_builder_api::WrapDatabaseRef;
use edr_blockchain_api::BlockHashByNumber;
use edr_chain_spec_evm::{
    interpreter::{
        CallInputs, CallOutcome, CreateInputs, CreateOutcome, EthInterpreter, Interpreter,
    },
    ContextTrait, Inspector, JournalTrait,
};
use edr_coverage::{reporter::SyncOnCollectedCoverageCallback, CodeCoverageReporter};
use edr_database_components::DatabaseComponents;
use edr_gas_report::SyncOnCollectedGasReportCallback;
use edr_inspector_bytecode::ExecutedBytecodeCollector;
use edr_primitives::{Address, Bytes, HashMap, HashSet};
use edr_solidity::{contract_decoder::ContractDecoder, tracing::SolidityTracingInspector};
use edr_state_api::State;
use foundry_evm_traces::CallTraceArena;
use parking_lot::RwLock;
use revm_inspector::JournalExt;
use revm_inspectors::tracing::{TracingInspector, TracingInspectorConfig};

use crate::{console_log::ConsoleLogCollector, mock::Mocker, SyncCallOverride};

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
#[derive(Clone)]
pub struct EvmObserverConfig {
    pub call_override: Option<Arc<dyn SyncCallOverride>>,
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

pub struct EvmObservedData {
    /// Mapping of contract address to executed bytecode
    pub address_to_executed_code: HashMap<Address, Bytes>,
    /// The call trace arena collected during execution, including ABI-decoded
    /// information.
    pub call_trace_arena: CallTraceArena,
    /// Encoded `console.log` call inputs
    pub encoded_console_logs: Vec<Bytes>,
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
            tracing_inspector: SolidityTracingInspector::new(
                TracingInspector::new(tracing_config),
                config.contract_decoder,
            ),
        }
    }

    /// Reports and collects the observed data of a single transaction.
    pub fn report_and_collect(
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
                .report()
                .map_err(EvmObserverCollectionError::OnCollectedCoverageCallback)?;
        }

        let address_to_executed_code = bytecode_collector.collect();
        let call_trace_arena = tracing_inspector
            .collect(&address_to_executed_code, &precompile_addresses)
            .map_err(EvmObserverCollectionError::AbiDecoding)?;

        Ok(EvmObservedData {
            address_to_executed_code,
            call_trace_arena,
            encoded_console_logs: console_logger.into_encoded_messages(),
        })
    }
}

impl<
        BlockchainT: BlockHashByNumber<Error: std::error::Error>,
        ContextT: ContextTrait<
            Journal: JournalExt
                         + JournalTrait<
                Database = WrapDatabaseRef<DatabaseComponents<BlockchainT, StateT>>,
            >,
        >,
        StateT: State<Error: std::error::Error>,
    > Inspector<ContextT, EthInterpreter> for EvmObserver
{
    fn call(&mut self, context: &mut ContextT, inputs: &mut CallInputs) -> Option<CallOutcome> {
        self.console_logger.call(context, inputs);
        if let Some(code_coverage) = &mut self.code_coverage {
            Inspector::<_, EthInterpreter>::call(&mut code_coverage.collector, context, inputs);
        }
        self.tracing_inspector.call(context, inputs);
        self.mocker.call(context, inputs)
    }

    fn call_end(&mut self, context: &mut ContextT, inputs: &CallInputs, outcome: &mut CallOutcome) {
        self.tracing_inspector.call_end(context, inputs, outcome);
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
    }

    fn step(&mut self, interp: &mut Interpreter<EthInterpreter>, context: &mut ContextT) {
        self.tracing_inspector.step(interp, context);
    }
}
