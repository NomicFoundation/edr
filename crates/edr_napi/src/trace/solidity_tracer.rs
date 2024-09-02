use edr_evm::interpreter::OpCode;
use edr_solidity::build_model::{Instruction, JumpType};
use napi::{
    bindgen_prelude::{Either3, Either4},
    Either,
};
use napi_derive::napi;

use super::{
    error_inferrer::{
        instruction_to_callstack_stack_trace_entry, ErrorInferrer, SubmessageDataRef,
    },
    mapped_inlined_internal_functions_heuristics::{
        adjust_stack_trace, stack_trace_may_require_adjustments,
    },
    message_trace::{CallMessageTrace, CreateMessageTrace, EvmStep, PrecompileMessageTrace},
    solidity_stack_trace::{PrecompileErrorStackTraceEntry, SolidityStackTrace},
};
use crate::trace::{
    exit::ExitCode,
    solidity_stack_trace::{
        ContractTooLargeErrorStackTraceEntry, SolidityStackTraceEntry, StackTraceEntryTypeConst,
        UnrecognizedContractCallstackEntryStackTraceEntry,
        UnrecognizedContractErrorStackTraceEntry, UnrecognizedCreateCallstackEntryStackTraceEntry,
        UnrecognizedCreateErrorStackTraceEntry,
    },
};

#[napi(constructor)]
pub struct SolidityTracer;

#[allow(clippy::unused_self)] // we allow this for convenience for now
#[napi]
impl SolidityTracer {
    #[napi(catch_unwind)]
    pub fn get_stack_trace(
        &self,
        trace: Either3<PrecompileMessageTrace, CallMessageTrace, CreateMessageTrace>,
    ) -> napi::Result<SolidityStackTrace> {
        let trace = match &trace {
            Either3::A(precompile) => Either3::A(precompile),
            Either3::B(call) => Either3::B(call),
            Either3::C(create) => Either3::C(create),
        };

        self.get_stack_trace_inner(trace)
    }

    pub fn get_stack_trace_inner(
        &self,
        trace: Either3<&PrecompileMessageTrace, &CallMessageTrace, &CreateMessageTrace>,
    ) -> napi::Result<SolidityStackTrace> {
        let exit = match &trace {
            Either3::A(precompile) => &precompile.exit,
            Either3::B(call) => &call.exit,
            Either3::C(create) => &create.exit,
        };

        if !exit.is_error() {
            return Ok(vec![]);
        }

        match trace {
            Either3::A(precompile) => Ok(self.get_precompile_message_stack_trace(precompile)?),
            Either3::B(call) if call.bytecode.is_some() => {
                Ok(self.get_call_message_stack_trace(call)?)
            }
            Either3::C(create) if create.bytecode.is_some() => {
                Ok(self.get_create_message_stack_trace(create)?)
            }
            // No bytecode is present
            Either3::B(call) => Ok(self.get_unrecognized_message_stack_trace(Either::A(call))?),
            Either3::C(create) => Ok(self.get_unrecognized_message_stack_trace(Either::B(create))?),
        }
    }

    fn get_last_subtrace<'a>(
        &self,
        trace: Either<&'a CallMessageTrace, &'a CreateMessageTrace>,
    ) -> Option<Either3<&'a PrecompileMessageTrace, &'a CallMessageTrace, &'a CreateMessageTrace>>
    {
        let (number_of_subtraces, steps) = match trace {
            Either::A(create) => (create.number_of_subtraces, &create.steps),
            Either::B(call) => (call.number_of_subtraces, &call.steps),
        };

        if number_of_subtraces == 0 {
            return None;
        }

        steps.iter().rev().find_map(|step| match step {
            Either4::A(EvmStep { .. }) => None,
            Either4::B(precompile) => Some(Either3::A(precompile)),
            Either4::C(call) => Some(Either3::B(call)),
            Either4::D(create) => Some(Either3::C(create)),
        })
    }

    fn get_precompile_message_stack_trace(
        &self,
        trace: &PrecompileMessageTrace,
    ) -> napi::Result<SolidityStackTrace> {
        Ok(vec![PrecompileErrorStackTraceEntry {
            type_: StackTraceEntryTypeConst,
            precompile: trace.precompile,
            source_reference: None,
        }
        .into()])
    }

    fn get_create_message_stack_trace(
        &self,
        trace: &CreateMessageTrace,
    ) -> napi::Result<SolidityStackTrace> {
        let inferred_error = ErrorInferrer::infer_before_tracing_create_message(trace)?;

        if let Some(inferred_error) = inferred_error {
            return Ok(inferred_error);
        }

        self.trace_evm_execution(Either::B(trace))
    }

    fn get_call_message_stack_trace(
        &self,
        trace: &CallMessageTrace,
    ) -> napi::Result<SolidityStackTrace> {
        let inferred_error = ErrorInferrer::infer_before_tracing_call_message(trace)?;

        if let Some(inferred_error) = inferred_error {
            return Ok(inferred_error);
        }

        self.trace_evm_execution(Either::A(trace))
    }

    fn get_unrecognized_message_stack_trace(
        &self,
        trace: Either<&CallMessageTrace, &CreateMessageTrace>,
    ) -> napi::Result<SolidityStackTrace> {
        let (trace_exit_kind, trace_return_data) = match &trace {
            Either::A(call) => (call.exit.kind(), &call.return_data),
            Either::B(create) => (create.exit.kind(), &create.return_data),
        };

        let subtrace = self.get_last_subtrace(trace);

        if let Some(subtrace) = subtrace {
            let (is_error, return_data) = match subtrace {
                Either3::A(precompile) => {
                    (precompile.exit.is_error(), precompile.return_data.clone())
                }
                Either3::B(call) => (call.exit.is_error(), call.return_data.clone()),
                Either3::C(create) => (create.exit.is_error(), create.return_data.clone()),
            };

            // This is not a very exact heuristic, but most of the time it will be right, as
            // solidity reverts if a call fails, and most contracts are in
            // solidity
            if is_error && trace_return_data.as_ref() == return_data.as_ref() {
                let unrecognized_entry: SolidityStackTraceEntry = match trace {
                    Either::A(CallMessageTrace { address, .. }) => {
                        UnrecognizedContractCallstackEntryStackTraceEntry {
                            type_: StackTraceEntryTypeConst,
                            address: address.clone(),
                            source_reference: None,
                        }
                        .into()
                    }
                    Either::B(CreateMessageTrace { .. }) => {
                        UnrecognizedCreateCallstackEntryStackTraceEntry {
                            type_: StackTraceEntryTypeConst,
                            source_reference: None,
                        }
                        .into()
                    }
                };

                let mut stacktrace = vec![unrecognized_entry];
                stacktrace.extend(self.get_stack_trace_inner(subtrace)?);

                return Ok(stacktrace);
            }
        }

        if trace_exit_kind == ExitCode::CODESIZE_EXCEEDS_MAXIMUM {
            return Ok(vec![ContractTooLargeErrorStackTraceEntry {
                type_: StackTraceEntryTypeConst,
                source_reference: None,
            }
            .into()]);
        }

        let is_invalid_opcode_error = trace_exit_kind == ExitCode::INVALID_OPCODE;

        match trace {
            Either::A(trace @ CallMessageTrace { .. }) => {
                Ok(vec![UnrecognizedContractErrorStackTraceEntry {
                    type_: StackTraceEntryTypeConst,
                    address: trace.address.clone(),
                    return_data: trace.return_data.clone(),
                    is_invalid_opcode_error,
                    source_reference: None,
                }
                .into()])
            }
            Either::B(trace @ CreateMessageTrace { .. }) => {
                Ok(vec![UnrecognizedCreateErrorStackTraceEntry {
                    type_: StackTraceEntryTypeConst,
                    return_data: trace.return_data.clone(),
                    is_invalid_opcode_error,
                    source_reference: None,
                }
                .into()])
            }
        }
    }

    fn trace_evm_execution(
        &self,
        trace: Either<&CallMessageTrace, &CreateMessageTrace>,
    ) -> napi::Result<SolidityStackTrace> {
        let stack_trace = self.raw_trace_evm_execution(trace)?;

        if stack_trace_may_require_adjustments(&stack_trace, trace) {
            return adjust_stack_trace(stack_trace, trace);
        }

        Ok(stack_trace)
    }

    fn raw_trace_evm_execution(
        &self,
        trace: Either<&CallMessageTrace, &CreateMessageTrace>,
    ) -> napi::Result<SolidityStackTrace> {
        let (bytecode, steps, number_of_subtraces) = match &trace {
            Either::A(call) => (&call.bytecode, &call.steps, call.number_of_subtraces),
            Either::B(create) => (&create.bytecode, &create.steps, create.number_of_subtraces),
        };
        let bytecode = bytecode.as_ref().expect("JS code asserts");

        let mut stacktrace: SolidityStackTrace = vec![];

        let mut subtraces_seen = 0;

        // There was a jump into a function according to the sourcemaps
        let mut jumped_into_function = false;

        let mut function_jumpdests: Vec<&Instruction> = vec![];

        let mut last_submessage_data: Option<SubmessageDataRef<'_>> = None;

        let mut iter = steps.iter().enumerate().peekable();
        while let Some((step_index, step)) = iter.next() {
            if let Either4::A(EvmStep { pc }) = step {
                let inst = bytecode.get_instruction(*pc)?;

                if inst.jump_type == JumpType::IntoFunction && iter.peek().is_some() {
                    let (_, next_step) = iter.peek().unwrap();
                    let Either4::A(next_evm_step) = next_step else {
                        unreachable!("JS code asserted that");
                    };
                    let next_inst = bytecode.get_instruction(next_evm_step.pc)?;

                    if next_inst.opcode == OpCode::JUMPDEST {
                        let frame = instruction_to_callstack_stack_trace_entry(bytecode, inst)?;
                        stacktrace.push(match frame {
                            Either::A(frame) => frame.into(),
                            Either::B(frame) => frame.into(),
                        });
                        if next_inst.location.is_some() {
                            jumped_into_function = true;
                        }
                        function_jumpdests.push(next_inst);
                    }
                } else if inst.jump_type == JumpType::OutofFunction {
                    stacktrace.pop();
                    function_jumpdests.pop();
                }
            } else {
                let message_trace = match step {
                    Either4::A(_) => unreachable!("branch is taken above"),
                    Either4::B(precompile) => Either3::A(precompile),
                    Either4::C(call) => Either3::B(call),
                    Either4::D(create) => Either3::C(create),
                };

                subtraces_seen += 1;

                // If there are more subtraces, this one didn't terminate the execution
                if subtraces_seen < number_of_subtraces {
                    continue;
                }

                let submessage_trace = self.get_stack_trace_inner(message_trace)?;

                last_submessage_data = Some(SubmessageDataRef {
                    message_trace,
                    step_index: step_index as u32,
                    stacktrace: submessage_trace,
                });
            }
        }

        let stacktrace_with_inferred_error = ErrorInferrer::infer_after_tracing(
            trace,
            stacktrace,
            &function_jumpdests,
            jumped_into_function,
            last_submessage_data,
        )?;

        ErrorInferrer::filter_redundant_frames(stacktrace_with_inferred_error)
    }
}
