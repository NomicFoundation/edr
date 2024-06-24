//! Naive Rust port of the `VMTracer` fromHardhat

use edr_eth::{Bytes, U256};
use edr_evm::{alloy_primitives::U160, ExecutionResult};

use crate::{
    exit::ExitCode,
    message_trace::{
        BaseEvmMessageTrace, BaseMessageTrace, CallMessageTrace, CreateMessageTrace, EvmStep,
        MessageTrace, MessageTraceStep, PrecompileMessageTrace,
    },
};
use edr_evm::trace::{BeforeMessage, Step};

pub struct VMTracer {
    pub tracing_steps: Vec<Step>,
    message_traces: Vec<MessageTrace>,
    last_error: Option<&'static str>,
    max_precompile_number: u64,
}

impl VMTracer {
    pub fn new() -> Self {
        // TODO: temporarily hardcoded to remove the need of using ethereumjs' common and evm here
        let max_precompile_number = 10;
        VMTracer {
            tracing_steps: vec![],
            message_traces: vec![],
            last_error: None,
            max_precompile_number,
        }
    }

    pub fn get_last_top_level_message_trace(&self) -> Option<&MessageTrace> {
        self.message_traces.first()
    }

    pub fn get_last_error(&self) -> Option<&'static str> {
        self.last_error
    }

    fn should_keep_tracing(&self) -> bool {
        self.last_error.is_none()
    }

    pub fn add_before_message(&mut self, message: BeforeMessage) {
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
            // TODO: Make this nicer and make sure the U160 logic/precompile comparison logic works
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

        if let Some(parent_trace) = self.message_traces.last_mut() {
            match parent_trace {
                MessageTrace::Precompile(_) => {
                    self.last_error = Some("This should not happen: message execution started while a precompile was executing");
                    return;
                }
                MessageTrace::Create(parent_trace) => {
                    parent_trace
                        .base
                        .steps
                        .push(MessageTraceStep::Message(trace.clone()));
                    parent_trace.base.number_of_subtraces += 1;
                }
                MessageTrace::Call(parent_trace) => {
                    parent_trace
                        .base
                        .steps
                        .push(MessageTraceStep::Message(trace.clone()));
                    parent_trace.base.number_of_subtraces += 1;
                }
            }
        }

        self.message_traces.push(trace);
    }

    pub fn add_step(&mut self, step: Step) {
        if !self.should_keep_tracing() {
            return;
        }

        if let Some(trace) = self.message_traces.last_mut() {
            let steps = match trace {
                MessageTrace::Precompile(_) => {
                    self.last_error = Some(
                        "This should not happen: step event fired while a precompile was executing",
                    );
                    return;
                }
                MessageTrace::Create(parent_trace) => &mut parent_trace.base.steps,
                MessageTrace::Call(parent_trace) => &mut parent_trace.base.steps,
            };

            steps.push(MessageTraceStep::Evm(EvmStep { pc: step.pc }));
        }

        self.tracing_steps.push(step);
    }

    pub fn add_after_message(&mut self, result: ExecutionResult) {
        if !self.should_keep_tracing() {
            return;
        }

        if let Some(trace) = self.message_traces.last_mut() {
            trace.base().gas_used = result.gas_used();

            match result {
                ExecutionResult::Success { reason, output, .. } => {
                    trace.base().exit = ExitCode::from(reason);
                    trace.base().return_data = output.data().clone();

                    if let MessageTrace::Create(trace) = trace {
                        let address = output
                            .address()
                            // The original code asserted this directly
                            .expect("address should be defined in create trace");

                        trace.deployed_contract = Some(Bytes::copy_from_slice(&address.0 .0));
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

            if self.message_traces.len() > 1 {
                self.message_traces.pop();
            }
        }
    }
}
