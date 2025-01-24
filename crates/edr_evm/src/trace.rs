mod context;
mod frame;

use std::fmt::Debug;

use derive_where::derive_where;
use edr_eth::{
    bytecode::opcode,
    result::{ExecutionResult, Output},
    spec::HaltReasonTrait,
    Address, Bytecode, Bytes, U256,
};
use revm::context_interface::Journal;

pub use self::{
    context::{TraceCollectorContext, TraceCollectorMutGetter},
    frame::TraceCollectorFrame,
};
use crate::{
    blockchain::BlockHash,
    interpreter::{
        return_revert, CallInputs, CallOutcome, CallValue, CreateInputs, CreateOutcome,
        EthInterpreter, Interpreter, Jumps as _, MemoryGetter as _, SuccessOrHalt,
    },
    state::{DatabaseComponents, State, WrapDatabaseRef},
};

/// Stack tracing message
#[derive(Clone, Debug)]
pub enum TraceMessage<HaltReasonT: HaltReasonTrait> {
    /// Event that occurs before a call or create message.
    Before(BeforeMessage),
    /// Event that occurs every step of a call or create message.
    Step(Step),
    /// Event that occurs after a call or create message.
    After(AfterMessage<HaltReasonT>),
}

/// Temporary before message type for handling traces
#[derive(Clone, Debug)]
pub struct BeforeMessage {
    /// Call depth
    pub depth: usize,
    /// Caller
    pub caller: Address,
    /// Callee
    pub to: Option<Address>,
    /// Whether the call is a static call
    pub is_static_call: bool,
    /// Transaction gas limit
    pub gas_limit: u64,
    /// Input data
    pub data: Bytes,
    /// Value
    pub value: U256,
    /// Code address
    pub code_address: Option<Address>,
    /// Bytecode
    pub code: Option<Bytecode>,
}

/// Event that occurs after a call or create message.
#[derive(Clone, Debug)]
pub struct AfterMessage<HaltReasonT: HaltReasonTrait> {
    /// The execution result
    pub execution_result: ExecutionResult<HaltReasonT>,
    /// The newly created contract address if it's a create tx. `None`
    /// if there was an error creating the contract.
    pub contract_address: Option<Address>,
}

/// A trace for an EVM call.
#[derive(Clone, Debug)]
#[derive_where(Default)]
pub struct Trace<HaltReasonT: HaltReasonTrait> {
    // /// The individual steps of the call
    // pub steps: Vec<Step>,
    /// Messages
    pub messages: Vec<TraceMessage<HaltReasonT>>,
    /// The return value of the call
    pub return_value: Bytes,
}

/// A single EVM step.
#[derive(Clone, Debug)]
pub struct Step {
    /// The program counter
    pub pc: u64,
    /// The call depth
    pub depth: u64,
    /// The executed op code
    pub opcode: u8,
    /// `Stack::Full` if verbose tracing is enabled, `Stack::Top` otherwise
    pub stack: Stack,
    /// Array of all allocated values. Only present if verbose tracing is
    /// enabled.
    pub memory: Option<Vec<u8>>,
    // /// The amount of gas that was used by the step
    // pub gas_cost: u64,
    // /// The amount of gas that was refunded by the step
    // pub gas_refunded: i64,
    // /// The contract being executed
    // pub contract: AccountInfo,
    // /// The address of the contract
    // pub contract_address: Address,
}

/// The stack at a step.
#[derive(Clone, Debug)]
pub enum Stack {
    /// The top of the stack at a step. None if the stack is empty.
    Top(Option<U256>),
    /// The full stack at a step.
    Full(Vec<U256>),
}

impl Stack {
    /// Get the top of the stack.
    pub fn top(&self) -> Option<&U256> {
        match self {
            Stack::Top(top) => top.as_ref(),
            Stack::Full(stack) => stack.last(),
        }
    }

    /// Get the full stack if it has been recorded.
    pub fn full(&self) -> Option<&Vec<U256>> {
        match self {
            Stack::Top(_) => None,
            Stack::Full(stack) => Some(stack),
        }
    }
}

impl<HaltReasonT: HaltReasonTrait> Trace<HaltReasonT> {
    /// Adds a before message
    pub fn add_before(&mut self, message: BeforeMessage) {
        self.messages.push(TraceMessage::Before(message));
    }

    /// Adds a result message
    pub fn add_after(&mut self, message: AfterMessage<HaltReasonT>) {
        self.messages.push(TraceMessage::After(message));
    }

    /// Adds a VM step to the trace
    pub fn add_step(&mut self, step: Step) {
        self.messages.push(TraceMessage::Step(step));
    }
}

/// Object that gathers trace information during EVM execution and can be turned
/// into a trace upon completion.
#[derive(Debug)]
pub struct TraceCollector<HaltReasonT: HaltReasonTrait> {
    traces: Vec<Trace<HaltReasonT>>,
    pending_before: Option<BeforeMessage>,
    is_new_trace: bool,
    verbose: bool,
}

impl<HaltReasonT: HaltReasonTrait> TraceCollector<HaltReasonT> {
    /// Create a trace collector. If verbose is `true` full stack and memory
    /// will be recorded.
    pub fn new(verbose: bool) -> Self {
        Self {
            traces: Vec::new(),
            pending_before: None,
            is_new_trace: true,
            verbose,
        }
    }

    /// Converts the [`TraceCollector`] into its [`Trace`].
    pub fn into_traces(self) -> Vec<Trace<HaltReasonT>> {
        self.traces
    }

    /// Returns the traces collected so far.
    pub fn traces(&self) -> &[Trace<HaltReasonT>] {
        &self.traces
    }

    fn current_trace_mut(&mut self) -> &mut Trace<HaltReasonT> {
        self.traces.last_mut().expect("Trace must have been added")
    }

    fn validate_before_message(&mut self) {
        if let Some(message) = self.pending_before.take() {
            self.current_trace_mut().add_before(message);
        }
    }

    /// Notifies the trace collector that a call is starting.
    pub fn notify_call_start<
        BlockchainT: BlockHash<Error: std::error::Error>,
        StateT: State<Error: std::error::Error>,
        FinalOutputT,
    >(
        &mut self,
        journal: &impl Journal<
            Database = WrapDatabaseRef<DatabaseComponents<BlockchainT, StateT>>,
            FinalOutput = FinalOutputT,
        >,
        inputs: &CallInputs,
    ) {
        if self.is_new_trace {
            self.is_new_trace = false;
            self.traces.push(Trace::default());
        }

        self.validate_before_message();

        let WrapDatabaseRef(DatabaseComponents { state, .. }) = journal.db_ref();

        // This needs to be split into two functions to avoid borrow checker issues
        #[allow(clippy::map_unwrap_or)]
        let code = journal
            .state()
            .get(&inputs.bytecode_address)
            .map(|account| account.info.clone())
            .map(|mut account_info| {
                if let Some(code) = account_info.code.take() {
                    code
                } else {
                    state.code_by_hash(account_info.code_hash).unwrap()
                }
            })
            .unwrap_or_else(|| {
                state.basic(inputs.bytecode_address).unwrap().map_or(
                    // If an invalid contract address was provided, return empty code
                    Bytecode::new(),
                    |account_info| {
                        account_info
                            .code
                            .unwrap_or_else(|| state.code_by_hash(account_info.code_hash).unwrap())
                    },
                )
            });

        self.pending_before = Some(BeforeMessage {
            depth: journal.depth(),
            caller: inputs.caller,
            to: Some(inputs.target_address),
            is_static_call: inputs.is_static,
            gas_limit: inputs.gas_limit,
            data: inputs.input.clone(),
            value: match inputs.value {
                CallValue::Transfer(value) | CallValue::Apparent(value) => value,
            },
            code_address: Some(inputs.bytecode_address),
            code: Some(code),
        });
    }

    /// Notifies the trace collector that a call has ended.
    pub fn notify_call_end<
        BlockchainT: BlockHash<Error: std::error::Error>,
        StateT: State<Error: std::error::Error>,
        FinalOutputT,
    >(
        &mut self,
        journal: &impl Journal<
            Database = WrapDatabaseRef<DatabaseComponents<BlockchainT, StateT>>,
            FinalOutput = FinalOutputT,
        >,
        outcome: &CallOutcome,
    ) {
        // TODO: Replace this with the `return_revert!` macro
        use crate::interpreter::InstructionResult;

        match outcome.instruction_result() {
            return_revert!() if self.pending_before.is_some() => {
                self.pending_before = None;
                return;
            }
            _ => (),
        }

        self.validate_before_message();

        let ret = *outcome.instruction_result();
        let safe_ret = if ret == InstructionResult::CallTooDeep
            || ret == InstructionResult::OutOfFunds
            || ret == InstructionResult::StateChangeDuringStaticCall
        {
            InstructionResult::Revert
        } else {
            ret
        };

        let execution_result = match SuccessOrHalt::from(safe_ret) {
            SuccessOrHalt::Success(reason) => ExecutionResult::Success {
                reason,
                gas_used: outcome.gas().spent(),
                gas_refunded: outcome.gas().refunded() as u64,
                logs: journal.logs().to_vec(),
                output: Output::Call(outcome.output().clone()),
            },
            SuccessOrHalt::Revert => ExecutionResult::Revert {
                gas_used: outcome.gas().spent(),
                output: outcome.output().clone(),
            },
            SuccessOrHalt::Halt(reason) => ExecutionResult::Halt {
                reason,
                gas_used: outcome.gas().limit(),
            },
            SuccessOrHalt::Internal(_) => {
                panic!("Internal error: {safe_ret:?}")
            }
            SuccessOrHalt::FatalExternalError => panic!("Fatal external error"),
        };

        self.current_trace_mut().add_after(AfterMessage {
            execution_result,
            contract_address: None,
        });
    }

    /// Notifies the trace collector that a create is starting.
    pub fn notify_create_start<
        BlockchainT: BlockHash<Error: std::error::Error>,
        StateT: State<Error: std::error::Error>,
        FinalOutputT,
    >(
        &mut self,
        journal: &impl Journal<
            Database = WrapDatabaseRef<DatabaseComponents<BlockchainT, StateT>>,
            FinalOutput = FinalOutputT,
        >,
        inputs: &CreateInputs,
    ) {
        if self.is_new_trace {
            self.is_new_trace = false;
            self.traces.push(Trace::default());
        }

        self.validate_before_message();

        self.pending_before = Some(BeforeMessage {
            depth: journal.depth(),
            caller: inputs.caller,
            to: None,
            gas_limit: inputs.gas_limit,
            is_static_call: false,
            data: inputs.init_code.clone(),
            value: inputs.value,
            code_address: None,
            code: None,
        });
    }

    /// Notifies the trace collector that a create has ended.
    pub fn notify_create_end<
        BlockchainT: BlockHash<Error: std::error::Error>,
        StateT: State<Error: std::error::Error>,
        FinalOutputT,
    >(
        &mut self,
        journal: &impl Journal<
            Database = WrapDatabaseRef<DatabaseComponents<BlockchainT, StateT>>,
            FinalOutput = FinalOutputT,
        >,
        outcome: &CreateOutcome,
    ) {
        // TODO: Replace this with the `return_revert!` macro
        use crate::interpreter::InstructionResult;

        self.validate_before_message();

        let ret = *outcome.instruction_result();
        let safe_ret =
            if ret == InstructionResult::CallTooDeep || ret == InstructionResult::OutOfFunds {
                InstructionResult::Revert
            } else {
                ret
            };

        let execution_result = match SuccessOrHalt::from(safe_ret) {
            SuccessOrHalt::Success(reason) => ExecutionResult::Success {
                reason,
                gas_used: outcome.gas().spent(),
                gas_refunded: outcome.gas().refunded() as u64,
                logs: journal.logs().to_vec(),
                output: Output::Create(outcome.output().clone(), outcome.address),
            },
            SuccessOrHalt::Revert => ExecutionResult::Revert {
                gas_used: outcome.gas().spent(),
                output: outcome.output().clone(),
            },
            SuccessOrHalt::Halt(reason) => ExecutionResult::Halt {
                reason,
                gas_used: outcome.gas().limit(),
            },
            SuccessOrHalt::Internal(_) => {
                panic!("Internal error: {safe_ret:?}")
            }
            SuccessOrHalt::FatalExternalError => panic!("Fatal external error"),
        };

        self.current_trace_mut().add_after(AfterMessage {
            execution_result,
            contract_address: outcome.address,
        });
    }

    /// Finishes the current trace.
    pub fn finish_trace(&mut self) {
        self.is_new_trace = true;
    }

    /// Notifies the trace collector that a step has started.
    pub fn notify_step_start<
        BlockchainT: BlockHash<Error: std::error::Error>,
        StateT: State<Error: std::error::Error>,
        FinalOutputT,
    >(
        &mut self,
        interpreter: &Interpreter<EthInterpreter>,
        journal: &impl Journal<
            Database = WrapDatabaseRef<DatabaseComponents<BlockchainT, StateT>>,
            FinalOutput = FinalOutputT,
        >,
    ) {
        // Skip the step
        let skip_step = self.pending_before.as_ref().map_or(false, |message| {
            message.code.is_some() && interpreter.bytecode.opcode() == opcode::STOP
        });

        self.validate_before_message();

        if !skip_step {
            let stack = if self.verbose {
                Stack::Full(interpreter.stack.data().clone())
            } else {
                Stack::Top(interpreter.stack.data().last().cloned())
            };
            let memory = if self.verbose {
                Some(
                    interpreter
                        .memory
                        .borrow()
                        .memory()
                        .context_memory()
                        .to_vec(),
                )
            } else {
                None
            };
            self.current_trace_mut().add_step(Step {
                pc: interpreter.bytecode.pc() as u64,
                depth: journal.depth() as u64,
                opcode: interpreter.bytecode.opcode(),
                stack,
                memory,
            });
        }
    }
}
