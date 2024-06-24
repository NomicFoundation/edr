//! Naive Rust port of the `VmTracer` from Hardhat.

use std::{cell::RefCell, rc::Rc};

use edr_eth::Bytes;
use edr_evm::{
    alloy_primitives::U160,
    trace::{BeforeMessage, Step},
    ExecutionResult,
};

use crate::{
    exit::ExitCode,
    message_trace::{
        BaseEvmMessageTrace, BaseMessageTrace, CallMessageTrace, CreateMessageTrace, EvmStep,
        MessageTrace, MessageTraceStep, PrecompileMessageTrace,
    },
};

type MessageTraceRefCell = Rc<RefCell<MessageTrace>>;

/// Naive Rust port of the `VmTracer` from Hardhat.
pub struct VmTracer {
    tracing_steps: Vec<Step>,
    message_traces: Vec<MessageTraceRefCell>,
    last_error: Option<&'static str>,
    max_precompile_number: u64,
}

impl Default for VmTracer {
    fn default() -> Self {
        // TODO: temporarily hardcoded to remove the need of using ethereumjs' common
        // and evm here
        let max_precompile_number = 10;
        VmTracer {
            tracing_steps: vec![],
            message_traces: vec![],
            last_error: None,
            max_precompile_number,
        }
    }
}

impl VmTracer {
    /// Creates a new [`VmTracer`].
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns a reference to the last top-level message trace.
    /// # Panics
    /// This function panics if executed concurrently with [`Self::observe`].
    pub fn get_last_top_level_message_trace(&self) -> Option<MessageTrace> {
        self.message_traces
            .last()
            .map(|x| RefCell::borrow(x).clone())
    }

    /// Returns a reference to the last top-level message trace.
    /// The reference is only being mutated for the duration of
    /// [`Self::observe`] call.
    pub fn get_last_top_level_message_trace_ref(&self) -> Option<&MessageTraceRefCell> {
        self.message_traces.first()
    }

    /// Retrieves the last error that occurred during the tracing process.
    pub fn get_last_error(&self) -> Option<&'static str> {
        self.last_error
    }

    /// Observes a trace, collecting information about the execution of the EVM.
    pub fn observe(&mut self, trace: &edr_evm::trace::Trace) {
        for msg in &trace.messages {
            match msg.clone() {
                edr_evm::trace::TraceMessage::Before(before) => {
                    self.add_before_message(before);
                }
                edr_evm::trace::TraceMessage::Step(step) => {
                    self.add_step(step);
                }
                edr_evm::trace::TraceMessage::After(after) => {
                    self.add_after_message(after.execution_result);
                }
            }
        }
    }

    fn should_keep_tracing(&self) -> bool {
        self.last_error.is_none()
    }

    fn add_before_message(&mut self, message: BeforeMessage) {
        if !self.should_keep_tracing() {
            return;
        }

        let trace: MessageTrace;

        if message.depth == 0 {
            self.message_traces.clear();
            self.tracing_steps.clear();
        }

        if message.to.is_none() {
            let create_trace = CreateMessageTrace {
                base: BaseEvmMessageTrace {
                    base: BaseMessageTrace {
                        depth: message.depth,
                        value: message.value,
                        exit: ExitCode::Success,
                        return_data: Bytes::new(),
                        gas_used: 0,
                    },
                    code: message.data,
                    steps: vec![],
                    number_of_subtraces: 0,
                    // this was not in the original code - assumed to be None/undefined?
                    bytecode: None,
                },

                deployed_contract: None,
            };

            trace = MessageTrace::Create(create_trace);
        } else {
            // TODO: Make this nicer and make sure the U160 logic/precompile comparison
            // logic works
            let to = message.to.unwrap();
            let to_as_bigint = U160::from_be_bytes(**to);

            if to_as_bigint <= U160::from(self.max_precompile_number) {
                let precompile_trace = PrecompileMessageTrace {
                    base: BaseMessageTrace {
                        value: message.value,
                        exit: ExitCode::Success,
                        return_data: Bytes::new(),
                        depth: message.depth,
                        gas_used: 0,
                    },
                    // The max precompile number is 10, so we can safely unwrap here
                    precompile: to_as_bigint.as_limbs()[0] as u32,
                    calldata: message.data,
                };

                trace = MessageTrace::Precompile(precompile_trace);
            } else {
                // if we enter here, then `to` is not None, therefore
                // `code_address` and `code` should be Some
                // TODO (if not true, then we need to return and set the error message)
                let code_address = if let Some(value) = message.code_address {
                    value
                } else {
                    self.last_error = Some("code_address should be Some");
                    return;
                };
                let code = if let Some(value) = message.code {
                    value
                } else {
                    self.last_error = Some("code should be Some");
                    return;
                };

                let call_trace = CallMessageTrace {
                    base: BaseEvmMessageTrace {
                        base: BaseMessageTrace {
                            depth: message.depth,
                            value: message.value,
                            exit: ExitCode::Success,
                            return_data: Bytes::new(),
                            gas_used: 0,
                        },
                        code: code.original_bytes(),
                        steps: vec![],
                        number_of_subtraces: 0,
                        // this was not in the original code - assumed to be None/undefined?
                        bytecode: None,
                    },
                    calldata: message.data,
                    address: message.to.unwrap(),
                    code_address,
                };

                trace = MessageTrace::Call(call_trace);
            }
        }

        // We need to share it so that adding steps when processing via stack
        // also updates the inner elements
        let trace = Rc::new(RefCell::new(trace));

        if let Some(parent_ref) = self.message_traces.last_mut() {
            let mut parent_trace = parent_ref.borrow_mut();
            let parent_trace = match &mut *parent_trace {
                MessageTrace::Precompile(_) => {
                    self.last_error = Some("This should not happen: message execution started while a precompile was executing");
                    return;
                }
                MessageTrace::Create(create) => &mut create.base,
                MessageTrace::Call(call) => &mut call.base,
            };

            parent_trace
                .steps
                .push(MessageTraceStep::Message(Rc::clone(&trace)));
            parent_trace.number_of_subtraces += 1;
        }

        self.message_traces.push(trace);
    }

    fn add_step(&mut self, step: Step) {
        if !self.should_keep_tracing() {
            return;
        }

        if let Some(parent_ref) = self.message_traces.last_mut() {
            let mut parent_trace = parent_ref.borrow_mut();
            let parent_trace = match &mut *parent_trace {
                MessageTrace::Precompile(_) => {
                    self.last_error = Some(
                        "This should not happen: step event fired while a precompile was executing",
                    );
                    return;
                }
                MessageTrace::Create(create) => &mut create.base,
                MessageTrace::Call(call) => &mut call.base,
            };

            parent_trace
                .steps
                .push(MessageTraceStep::Evm(EvmStep { pc: step.pc }));
        }

        self.tracing_steps.push(step);
    }

    fn add_after_message(&mut self, result: ExecutionResult) {
        if !self.should_keep_tracing() {
            return;
        }

        if let Some(trace) = self.message_traces.last_mut() {
            let mut trace = trace.borrow_mut();

            trace.base().gas_used = result.gas_used();

            match result {
                ExecutionResult::Success { output, .. } => {
                    trace.base().exit = ExitCode::Success;
                    trace.base().return_data = output.data().clone();

                    if let MessageTrace::Create(trace) = &mut *trace {
                        let address = output
                            .address()
                            // The original code asserted this directly
                            .expect("address should be defined in create trace");

                        trace.deployed_contract = Some(address.as_slice().to_vec().into());
                    }
                }
                ExecutionResult::Halt { reason, .. } => {
                    trace.base().exit = ExitCode::from(reason);
                    trace.base().return_data = Bytes::new();
                }
                ExecutionResult::Revert { output, .. } => {
                    trace.base().exit = ExitCode::Revert;
                    trace.base().return_data = output;
                }
            }
        }

        if self.message_traces.len() > 1 {
            self.message_traces.pop();
        }
    }
}
