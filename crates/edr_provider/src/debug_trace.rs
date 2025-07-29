use std::{collections::HashMap, fmt::Debug};

use edr_eth::{
    block::Block as _,
    bytecode::opcode::{self, OpCode},
    hex, l1,
    result::{ExecutionResult, ExecutionResultAndState},
    spec::{ChainSpec, HaltReasonTrait},
    transaction::{ExecutableTransaction as _, TransactionValidation},
    utils::u256_to_padded_hex,
    Address, Bytes, B256, U256,
};
use edr_evm::{
    blockchain::SyncBlockchain,
    config::CfgEnv,
    inspector::{DualInspector, Inspector},
    interpreter::{
        CallInputs, CallOutcome, CreateInputs, CreateOutcome, EthInterpreter,
        InputsTr as _, Interpreter, InterpreterResult, Jumps as _, LoopControl as _,
    },
    journal::{JournalEntry, JournalExt, JournalTrait as _},
    runtime::{dry_run_with_inspector, run},
    spec::{ContextTrait, RuntimeSpec},
    state::SyncState,
    trace::{Trace, TraceCollector},
    transaction::TransactionError,
};

use crate::observability::{self, RuntimeObserver};

/// Get trace output for `debug_traceTransaction`
#[cfg_attr(feature = "tracing", tracing::instrument(skip_all))]
#[allow(clippy::too_many_arguments)]
pub fn debug_trace_transaction<ChainSpecT, BlockchainErrorT, StateErrorT>(
    blockchain: &dyn SyncBlockchain<ChainSpecT, BlockchainErrorT, StateErrorT>,
    // Take ownership of the state so that we can apply throw-away modifications on it
    mut state: Box<dyn SyncState<StateErrorT>>,
    evm_config: CfgEnv<ChainSpecT::Hardfork>,
    trace_config: DebugTraceConfig,
    block: ChainSpecT::BlockEnv,
    transactions: Vec<ChainSpecT::SignedTransaction>,
    transaction_hash: &B256,
    observability: observability::Config,
) -> Result<
    DebugTraceResultWithTraces<ChainSpecT::HaltReason>,
    DebugTraceErrorForChainSpec<BlockchainErrorT, ChainSpecT, StateErrorT>,
>
where
    ChainSpecT: RuntimeSpec<
        BlockEnv: Clone,
        SignedTransaction: Default
                               + TransactionValidation<ValidationError: From<l1::InvalidTransaction>>,
    >,
    BlockchainErrorT: Send + std::error::Error,
    StateErrorT: Send + std::error::Error,
{
    let evm_spec_id = evm_config.spec.into();
    if evm_spec_id < l1::SpecId::SPURIOUS_DRAGON {
        // Matching Hardhat Network behaviour: https://github.com/NomicFoundation/hardhat/blob/af7e4ce6a18601ec9cd6d4aa335fa7e24450e638/packages/hardhat-core/src/internal/hardhat-network/provider/vm/ethereumjs.ts#L427
        return Err(DebugTraceError::InvalidSpecId {
            spec_id: evm_spec_id,
        });
    }

    let block_number = block.number();
    for transaction in transactions {
        if transaction.transaction_hash() == transaction_hash {
            let mut eip3155_tracer = TracerEip3155::new(trace_config);
            let mut runtime_observer = RuntimeObserver::new(observability);

            let ExecutionResultAndState { result, .. } =
                dry_run_with_inspector::<_, ChainSpecT, _, _>(
                    blockchain,
                    state.as_ref(),
                    evm_config,
                    transaction,
                    block,
                    &edr_eth::HashMap::new(),
                    &mut DualInspector::new(&mut eip3155_tracer, &mut runtime_observer),
                )?;

            let RuntimeObserver {
                code_coverage,
                console_logger: _console_logger,
                mocker: _mocker,
                trace_collector,
            } = runtime_observer;

            if let Some(code_coverage) = code_coverage {
                code_coverage
                    .report()
                    .map_err(DebugTraceError::OnCollectedCoverageCallback)?;
            }

            return Ok(execution_result_to_debug_result(
                result,
                trace_collector,
                eip3155_tracer,
            ));
        } else {
            run::<_, ChainSpecT, _>(
                blockchain,
                state.as_mut(),
                evm_config.clone(),
                transaction,
                block.clone(),
                &edr_eth::HashMap::new(),
            )?;
        }
    }

    Err(DebugTraceError::InvalidTransactionHash {
        transaction_hash: *transaction_hash,
        block_number,
    })
}

/// Convert an `ExecutionResult` to a `DebugTraceResult`.
pub fn execution_result_to_debug_result<HaltReasonT: HaltReasonTrait>(
    execution_result: ExecutionResult<HaltReasonT>,
    raw_tracer: TraceCollector<HaltReasonT>,
    eip3155_tracer: TracerEip3155,
) -> DebugTraceResultWithTraces<HaltReasonT> {
    let traces = raw_tracer.into_traces();

    let result = match execution_result {
        ExecutionResult::Success {
            gas_used, output, ..
        } => DebugTraceResult {
            pass: true,
            gas_used,
            output: Some(output.into_data()),
            logs: eip3155_tracer.logs,
        },
        ExecutionResult::Revert { gas_used, output } => DebugTraceResult {
            pass: false,
            gas_used,
            output: Some(output),
            logs: eip3155_tracer.logs,
        },
        ExecutionResult::Halt { gas_used, .. } => DebugTraceResult {
            pass: false,
            gas_used,
            output: None,
            logs: eip3155_tracer.logs,
        },
    };

    DebugTraceResultWithTraces { result, traces }
}

/// Config options for `debug_traceTransaction`
#[derive(Debug, Default, Clone)]
pub struct DebugTraceConfig {
    /// Disable storage trace.
    pub disable_storage: bool,
    /// Disable memory trace.
    pub disable_memory: bool,
    /// Disable stack trace.
    pub disable_stack: bool,
}

/// Helper type for a chain-specific [`DebugTraceError`].
pub type DebugTraceErrorForChainSpec<BlockchainErrorT, ChainSpecT, StateErrorT> = DebugTraceError<
    BlockchainErrorT,
    StateErrorT,
    <<ChainSpecT as ChainSpec>::SignedTransaction as TransactionValidation>::ValidationError,
>;

/// Debug trace error.
#[derive(Debug, thiserror::Error)]
pub enum DebugTraceError<BlockchainErrorT, StateErrorT, TransactionValidationErrorT> {
    /// Invalid hardfork spec argument.
    #[error(
        "Invalid spec id: {spec_id:?}. `debug_traceTransaction` is not supported prior to Spurious Dragon"
    )]
    InvalidSpecId {
        /// The hardfork.
        spec_id: l1::SpecId,
    },
    /// Invalid transaction hash argument.
    #[error("Transaction hash {transaction_hash} not found in block {block_number}")]
    InvalidTransactionHash {
        /// The transaction hash.
        transaction_hash: B256,
        /// The block number.
        block_number: u64,
    },
    /// An error occurred while invoking a `SyncOnCollectedCoverageCallback`.
    #[error(transparent)]
    OnCollectedCoverageCallback(Box<dyn std::error::Error + Send + Sync>),
    /// Transaction error.
    #[error(transparent)]
    TransactionError(
        #[from] TransactionError<BlockchainErrorT, StateErrorT, TransactionValidationErrorT>,
    ),
}

/// Result of a `debug_traceTransaction` call.
#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DebugTraceResult {
    /// Whether transaction was executed successfully.
    pub pass: bool,
    /// All gas used by the transaction.
    pub gas_used: u64,
    /// Return values of the function.
    pub output: Option<Bytes>,
    /// The EIP-3155 debug logs.
    #[serde(rename = "structLogs")]
    pub logs: Vec<DebugTraceLogItem>,
}

/// Result of a `debug_traceTransaction` call with traces.
pub struct DebugTraceResultWithTraces<HaltReasonT: HaltReasonTrait> {
    /// The result of the transaction.
    pub result: DebugTraceResult,
    /// The raw traces of the debugged transaction.
    pub traces: Vec<Trace<HaltReasonT>>,
}

/// The output of an EIP-3155 trace.
/// The required fields match <https://eips.ethereum.org/EIPS/eip-3155#output> except for
/// `returnData` and `refund` which are not used currently by Hardhat.
/// The `opName`, `error`, `memory` and `storage` optional fields are supported
/// as well.
#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DebugTraceLogItem {
    /// Program Counter
    pub pc: u64,
    /// Op code
    pub op: u8,
    /// Gas left before executing this operation as hex number.
    pub gas: String,
    /// Gas cost of this operation as hex number.
    pub gas_cost: String,
    /// Array of all values (hex numbers) on the stack
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stack: Option<Vec<String>>,
    /// Depth of the call stack
    pub depth: u64,
    /// Size of memory array.
    pub mem_size: u64,
    /// Name of the operation.
    pub op_name: String,
    /// Description of an error.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    /// Array of all allocated values as hex strings.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub memory: Option<Vec<String>>,
    /// Map of all stored values with keys and values encoded as hex strings.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub storage: Option<HashMap<String, String>>,
}

/// An EIP-3155 compatible EVM tracer.
#[derive(Debug)]
pub struct TracerEip3155 {
    config: DebugTraceConfig,
    logs: Vec<DebugTraceLogItem>,
    contract_address: Address,
    gas_remaining: u64,
    memory: Vec<u8>,
    mem_size: usize,
    opcode: u8,
    pc: usize,
    stack: Vec<U256>,
    // Contract-specific storage
    storage: HashMap<Address, HashMap<String, String>>,
}

impl TracerEip3155 {
    /// Create a new tracer.
    pub fn new(config: DebugTraceConfig) -> Self {
        Self {
            config,
            logs: Vec::default(),
            contract_address: Address::default(),
            stack: Vec::new(),
            pc: 0,
            opcode: 0,
            gas_remaining: 0,
            memory: Vec::default(),
            mem_size: 0,
            storage: HashMap::default(),
        }
    }

    fn on_inner_frame_result(&mut self, result: &InterpreterResult) {
        self.gas_remaining = if result.result.is_error() {
            0
        } else {
            result.gas.remaining()
        };
    }
}

impl<ContextT: ContextTrait<Journal: JournalExt<Entry = JournalEntry>>> Inspector<ContextT>
    for TracerEip3155
{
    fn call_end(
        &mut self,
        _context: &mut ContextT,
        _inputs: &CallInputs,
        outcome: &mut CallOutcome,
    ) {
        self.on_inner_frame_result(&outcome.result);
    }

    fn create_end(
        &mut self,
        _context: &mut ContextT,
        _inputs: &CreateInputs,
        outcome: &mut CreateOutcome,
    ) {
        self.on_inner_frame_result(&outcome.result);
    }

    fn step(&mut self, interpreter: &mut Interpreter<EthInterpreter>, _context: &mut ContextT) {
        self.contract_address = interpreter.input.target_address();
        self.gas_remaining = interpreter.control.gas().remaining();

        if !self.config.disable_stack {
            self.stack.clone_from(interpreter.stack.data());
        }

        let shared_memory = &interpreter.memory;
        if !self.config.disable_memory {
            self.memory = shared_memory.context_memory().to_vec();
        }

        self.mem_size = shared_memory.context_memory().len();

        self.opcode = interpreter.bytecode.opcode();
        self.pc = interpreter.bytecode.pc();
    }

    fn step_end(&mut self, interpreter: &mut Interpreter<EthInterpreter>, context: &mut ContextT) {
        let journal = context.journal();
        let depth = journal.depth() as u64;

        let stack = if self.config.disable_stack {
            None
        } else {
            Some(
                self.stack
                    .iter()
                    .map(u256_to_padded_hex)
                    .collect::<Vec<String>>(),
            )
        };

        let memory = if self.config.disable_memory {
            None
        } else {
            Some(self.memory.chunks(32).map(hex::encode).collect())
        };

        let storage = if self.config.disable_storage {
            None
        } else {
            if matches!(self.opcode, opcode::SLOAD | opcode::SSTORE) {
                let last_entry = journal.entries().last();

                if let Some(
                    JournalEntry::StorageChanged { address, key, .. }
                    | JournalEntry::StorageWarmed { address, key },
                ) = last_entry
                {
                    let value = journal.state()[address].storage[key].present_value();
                    let contract_storage = self.storage.entry(self.contract_address).or_default();
                    contract_storage.insert(u256_to_padded_hex(key), u256_to_padded_hex(&value));
                }
            }
            Some(
                self.storage
                    .get(&self.contract_address)
                    .cloned()
                    .unwrap_or_default(),
            )
        };

        let mut error = None;
        let op_name = OpCode::new(self.opcode).map_or_else(
            || {
                // Matches message from Hardhat
                // https://github.com/NomicFoundation/hardhat/blob/37c5c5845969b15995cc96cb6bd0596977f8b1f8/packages/hardhat-core/src/internal/hardhat-network/stack-traces/vm-debug-tracer.ts#L452
                let fallback = format!("opcode 0x${:x} not defined", self.opcode);
                error = Some(fallback.clone());
                fallback
            },
            |opcode| opcode.to_string(),
        );

        let gas_cost = self
            .gas_remaining
            .saturating_sub(interpreter.control.gas().remaining());
        let log_item = DebugTraceLogItem {
            pc: self.pc as u64,
            op: self.opcode,
            gas: format!("0x{:x}", self.gas_remaining),
            gas_cost: format!("0x{gas_cost:x}"),
            stack,
            depth,
            mem_size: self.mem_size as u64,
            op_name,
            error,
            memory,
            storage,
        };
        self.logs.push(log_item);
    }
}
