//! Types used for tracing EVM calls
#![warn(missing_docs)]

use std::fmt::Debug;

use derive_where::derive_where;
use edr_block_builder_api::WrapDatabaseRef;
use edr_blockchain_api::BlockHashByNumber;
use edr_chain_spec::HaltReasonTrait;
use edr_chain_spec_evm::{
    interpreter::{
        return_revert, CallInputs, CallOutcome, CallValue, CreateInputs, CreateOutcome,
        EthInterpreter, Gas, Interpreter, Jumps as _, SuccessOrHalt,
    },
    result::{Output, SuccessReason},
    ContextTrait, Inspector, JournalTrait,
};
use edr_database_components::DatabaseComponents;
use edr_primitives::{bytecode::opcode, Address, Bytecode, Bytes, U256};
use edr_receipt::log::ExecutionLog;
use edr_state_api::State;
use revm_inspector::JournalExt;

/// Gas of a single call or create message, as charged to its caller.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MessageGas {
    /// Gas limit of the message.
    pub limit: u64,
    /// Gas spent by the message, including state gas (EIP-8037).
    pub spent: u64,
    /// Refund recorded by the message. Negative when the message revoked
    /// refunds earned by a parent message.
    pub refunded: i64,
    /// State gas spent by the message (EIP-8037). Negative when the message
    /// refilled state gas charged by a parent message.
    pub state_gas_spent: i64,
}

impl From<&Gas> for MessageGas {
    fn from(gas: &Gas) -> Self {
        Self {
            limit: gas.limit(),
            spent: gas.total_gas_spent(),
            refunded: gas.refunded(),
            state_gas_spent: gas.state_gas_spent(),
        }
    }
}

impl MessageGas {
    /// Gas that spent its whole limit and has no refund, as charged for a
    /// halted message.
    fn exhausted(self) -> Self {
        Self {
            spent: self.limit,
            refunded: 0,
            ..self
        }
    }

    /// Gas spent net of the refund.
    pub const fn used(&self) -> u64 {
        self.spent.saturating_sub_signed(self.refunded)
    }
}

/// Result of a single call or create message.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MessageResult<HaltReasonT: HaltReasonTrait> {
    /// The gas of the message.
    pub gas: MessageGas,
    /// The logs emitted so far in the transaction.
    pub logs: Vec<ExecutionLog>,
    /// How the message exited.
    pub exit: MessageExit<HaltReasonT>,
}

/// How a call or create message exited.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MessageExit<HaltReasonT: HaltReasonTrait> {
    /// The message returned successfully.
    Success {
        /// The reason for termination.
        reason: SuccessReason,
        /// The output of the message.
        output: Output,
    },
    /// The message reverted.
    Revert {
        /// The revert data.
        output: Bytes,
    },
    /// The message halted.
    Halt {
        /// The reason for the halt.
        reason: HaltReasonT,
    },
}

impl<HaltReasonT: HaltReasonTrait> MessageResult<HaltReasonT> {
    /// Builds the result of a message from its gas accumulator at exit. A
    /// halted message is charged its whole gas limit.
    pub fn new(gas: &Gas, logs: Vec<ExecutionLog>, exit: MessageExit<HaltReasonT>) -> Self {
        let gas = match exit {
            MessageExit::Halt { .. } => MessageGas::from(gas).exhausted(),
            MessageExit::Success { .. } | MessageExit::Revert { .. } => gas.into(),
        };

        Self { gas, logs, exit }
    }
}

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
    /// The result of the message
    pub result: MessageResult<HaltReasonT>,
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
    pub pc: u32,
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
    frame_depth: usize,
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
            frame_depth: 0,
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
            self.frame_depth += 1;
        }
    }

    fn add_after_message(&mut self, message: AfterMessage<HaltReasonT>) {
        self.current_trace_mut().add_after(message);
        self.frame_depth -= 1;

        if self.frame_depth == 0 {
            self.finish_trace();
        }
    }

    /// Notifies the trace collector that a call is starting.
    pub fn notify_call_start<
        BlockchainT: BlockHashByNumber<Error: 'static + std::error::Error + Send + Sync>,
        JournalT: JournalExt
            + JournalTrait<Database = WrapDatabaseRef<DatabaseComponents<BlockchainT, StateT>>>,
        StateT: State<Error: 'static + std::error::Error + Send + Sync>,
    >(
        &mut self,
        journal: &JournalT,
        inputs: &CallInputs,
        input_data: Bytes,
    ) {
        if self.is_new_trace {
            self.is_new_trace = false;
            self.traces.push(Trace::default());
        }

        self.validate_before_message();

        let WrapDatabaseRef(DatabaseComponents { state, .. }) = journal.db();

        // This needs to be split into two functions to avoid borrow checker issues
        #[allow(clippy::map_unwrap_or)]
        let code = journal
            .evm_state()
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
            data: input_data,
            value: match inputs.value {
                CallValue::Transfer(value) | CallValue::Apparent(value) => value,
            },
            code_address: Some(inputs.bytecode_address),
            code: Some(code),
        });
    }

    /// Notifies the trace collector that a call has ended.
    pub fn notify_call_end<
        BlockchainT: BlockHashByNumber<Error: 'static + std::error::Error + Send + Sync>,
        ContextT: ContextTrait<
            Journal: JournalExt
                         + JournalTrait<
                Database = WrapDatabaseRef<DatabaseComponents<BlockchainT, StateT>>,
            >,
        >,
        StateT: State<Error: 'static + std::error::Error + Send + Sync>,
    >(
        &mut self,
        context: &mut ContextT,
        outcome: &CallOutcome,
    ) {
        // TODO: Replace this with the `return_revert!` macro
        use edr_chain_spec_evm::interpreter::InstructionResult;

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

        let exit = match SuccessOrHalt::from(safe_ret) {
            SuccessOrHalt::Success(reason) => MessageExit::Success {
                reason,
                output: Output::Call(outcome.output().clone()),
            },
            SuccessOrHalt::Revert => MessageExit::Revert {
                output: outcome.output().clone(),
            },
            SuccessOrHalt::Halt(reason) => MessageExit::Halt { reason },
            SuccessOrHalt::Internal(_) => {
                panic!("Internal error: {safe_ret:?}")
            }
            SuccessOrHalt::FatalExternalError => {
                panic!("Fatal external error: {error:?}", error = context.error())
            }
        };

        self.add_after_message(AfterMessage {
            result: MessageResult::new(&outcome.gas(), context.journal().logs().to_vec(), exit),
            contract_address: None,
        });
    }

    /// Notifies the trace collector that a create is starting.
    pub fn notify_create_start<
        BlockchainT: BlockHashByNumber<Error: 'static + std::error::Error + Send + Sync>,
        StateT: State<Error: 'static + std::error::Error + Send + Sync>,
        FinalOutputT,
    >(
        &mut self,
        journal: &impl JournalTrait<
            Database = WrapDatabaseRef<DatabaseComponents<BlockchainT, StateT>>,
            State = FinalOutputT,
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
            caller: inputs.caller(),
            to: None,
            gas_limit: inputs.gas_limit(),
            is_static_call: false,
            data: inputs.init_code().clone(),
            value: inputs.value(),
            code_address: None,
            code: None,
        });
    }

    /// Notifies the trace collector that a create has ended.
    pub fn notify_create_end<
        BlockchainT: BlockHashByNumber<Error: 'static + std::error::Error + Send + Sync>,
        ContextT: ContextTrait<
            Journal: JournalExt
                         + JournalTrait<
                Database = WrapDatabaseRef<DatabaseComponents<BlockchainT, StateT>>,
            >,
        >,
        StateT: State<Error: 'static + std::error::Error + Send + Sync>,
    >(
        &mut self,
        context: &mut ContextT,
        outcome: &CreateOutcome,
    ) {
        // TODO: Replace this with the `return_revert!` macro
        use edr_chain_spec_evm::interpreter::InstructionResult;

        self.validate_before_message();

        let ret = *outcome.instruction_result();
        let safe_ret =
            if ret == InstructionResult::CallTooDeep || ret == InstructionResult::OutOfFunds {
                InstructionResult::Revert
            } else {
                ret
            };

        let exit = match SuccessOrHalt::from(safe_ret) {
            SuccessOrHalt::Success(reason) => MessageExit::Success {
                reason,
                output: Output::Create(outcome.output().clone(), outcome.address),
            },
            SuccessOrHalt::Revert => MessageExit::Revert {
                output: outcome.output().clone(),
            },
            SuccessOrHalt::Halt(reason) => MessageExit::Halt { reason },
            SuccessOrHalt::Internal(error) => {
                panic!("Internal error: {error:?}")
            }
            SuccessOrHalt::FatalExternalError => {
                panic!("Fatal external error: {error:?}", error = context.error())
            }
        };

        self.add_after_message(AfterMessage {
            result: MessageResult::new(outcome.gas(), context.journal().logs().to_vec(), exit),
            contract_address: outcome.address,
        });
    }

    /// Finishes the current trace.
    pub fn finish_trace(&mut self) {
        self.is_new_trace = true;
    }

    /// Notifies the trace collector that a step has started.
    pub fn notify_step_start<
        BlockchainT: BlockHashByNumber<Error: 'static + std::error::Error + Send + Sync>,
        StateT: State<Error: 'static + std::error::Error + Send + Sync>,
        FinalOutputT,
    >(
        &mut self,
        interpreter: &Interpreter<EthInterpreter>,
        journal: &impl JournalTrait<
            Database = WrapDatabaseRef<DatabaseComponents<BlockchainT, StateT>>,
            State = FinalOutputT,
        >,
    ) {
        // Skip the step
        let skip_step = self.pending_before.as_ref().is_some_and(|message| {
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
                Some(interpreter.memory.context_memory().to_vec())
            } else {
                None
            };

            let pc = interpreter
                .bytecode
                .pc()
                .try_into()
                .expect("Program Counter should fit inside u32");

            self.current_trace_mut().add_step(Step {
                pc,
                depth: journal.depth() as u64,
                opcode: interpreter.bytecode.opcode(),
                stack,
                memory,
            });
        }
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
        HaltReasonT: HaltReasonTrait,
        StateT: State<Error: 'static + std::error::Error + Send + Sync>,
    > Inspector<ContextT, EthInterpreter> for TraceCollector<HaltReasonT>
{
    fn call(&mut self, context: &mut ContextT, inputs: &mut CallInputs) -> Option<CallOutcome> {
        let input_data = inputs.input.bytes(context);
        self.notify_call_start(context.journal(), inputs, input_data);
        None
    }

    fn call_end(
        &mut self,
        context: &mut ContextT,
        _inputs: &CallInputs,
        outcome: &mut CallOutcome,
    ) {
        self.notify_call_end(context, outcome);
    }

    fn create(
        &mut self,
        context: &mut ContextT,
        inputs: &mut CreateInputs,
    ) -> Option<CreateOutcome> {
        self.notify_create_start(context.journal(), inputs);
        None
    }

    fn create_end(
        &mut self,
        context: &mut ContextT,
        _inputs: &CreateInputs,
        outcome: &mut CreateOutcome,
    ) {
        self.notify_create_end(context, outcome);
    }

    fn step(&mut self, interpreter: &mut Interpreter<EthInterpreter>, context: &mut ContextT) {
        self.notify_step_start(interpreter, context.journal());
    }
}

#[cfg(test)]
mod tests {
    use edr_chain_spec::EvmHaltReason;

    use super::*;

    fn gas(limit: u64, cost: u64, refund: i64, state_gas: i64) -> Gas {
        let mut gas = Gas::new(limit);
        assert!(gas.record_regular_cost(cost));
        gas.record_refund(refund);
        gas.set_state_gas_spent(state_gas);
        gas
    }

    #[test]
    fn from_gas_keeps_signed_counters() {
        let frame = MessageGas::from(&gas(100_000, 40_000, -4_800, -20_000));

        assert_eq!(
            frame,
            MessageGas {
                limit: 100_000,
                spent: 40_000,
                refunded: -4_800,
                state_gas_spent: -20_000,
            }
        );
    }

    #[test]
    fn new_charges_whole_limit_on_halt() {
        let result = MessageResult::new(
            &gas(100_000, 40_000, 4_800, 20_000),
            Vec::new(),
            MessageExit::<EvmHaltReason>::Halt {
                reason: EvmHaltReason::OpcodeNotFound,
            },
        );

        assert_eq!(
            result.gas,
            MessageGas {
                limit: 100_000,
                spent: 100_000,
                refunded: 0,
                state_gas_spent: 20_000,
            }
        );
    }

    #[test]
    fn new_keeps_gas_on_revert() {
        let gas = gas(100_000, 40_000, 4_800, 20_000);
        let result = MessageResult::new(
            &gas,
            Vec::new(),
            MessageExit::<EvmHaltReason>::Revert {
                output: Bytes::new(),
            },
        );

        assert_eq!(result.gas, MessageGas::from(&gas));
    }

    #[test]
    fn used_subtracts_refund() {
        let frame = MessageGas::from(&gas(100_000, 40_000, 4_800, 0));
        assert_eq!(frame.used(), 35_200);
    }

    #[test]
    fn used_saturates_when_refund_exceeds_spent() {
        let frame = MessageGas::from(&gas(100_000, 2_900, 4_800, 0));
        assert_eq!(frame.used(), 0);
    }

    #[test]
    fn used_adds_revoked_refund() {
        let frame = MessageGas::from(&gas(100_000, 40_000, -4_800, 0));
        assert_eq!(frame.used(), 44_800);
    }
}
