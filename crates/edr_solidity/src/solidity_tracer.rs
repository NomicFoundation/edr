use edr_evm::interpreter::OpCode;
use either::Either;

use crate::{
    build_model::{BytecodeError, Instruction, JumpType},
    error_inferrer,
    error_inferrer::{instruction_to_callstack_stack_trace_entry, InferrerError, SubmessageData},
    mapped_inline_internal_functions_heuristics::{
        adjust_stack_trace, stack_trace_may_require_adjustments, HeuristicsError,
    },
    message_trace::{
        CallMessageTrace, CreateMessageTrace, EvmStep, MessageTrace, MessageTraceStep,
        PrecompileMessageTrace,
    },
    solidity_stack_trace::StackTraceEntry,
};

pub struct SolidityTracer;

#[derive(Debug, thiserror::Error)]
pub enum SolidityTracerError {
    #[error(transparent)]
    BytecodeError(#[from] BytecodeError),
    #[error(transparent)]
    ErrorInferrer(#[from] InferrerError),
    #[error(transparent)]
    Heuristics(#[from] HeuristicsError),
}

pub fn get_stack_trace(trace: MessageTrace) -> Result<Vec<StackTraceEntry>, SolidityTracerError> {
    if !trace.exit().is_error() {
        return Ok(vec![]);
    }

    match trace {
        MessageTrace::Precompile(precompile) => {
            Ok(get_precompile_message_stack_trace(&precompile)?)
        }
        MessageTrace::Call(call) if call.bytecode().is_some() => {
            Ok(get_call_message_stack_trace(call)?)
        }
        MessageTrace::Create(create) if create.bytecode().is_some() => {
            Ok(get_create_message_stack_trace(create)?)
        }
        // No bytecode is present
        MessageTrace::Call(call) => Ok(get_unrecognized_message_stack_trace(Either::Left(call))?),
        MessageTrace::Create(create) => {
            Ok(get_unrecognized_message_stack_trace(Either::Right(create))?)
        }
    }
}

fn get_last_subtrace<'a>(
    trace: &'a Either<CallMessageTrace, CreateMessageTrace>,
) -> Option<MessageTrace> {
    let (number_of_subtraces, steps) = match trace {
        Either::Left(create) => (create.number_of_subtraces(), create.steps()),
        Either::Right(call) => (call.number_of_subtraces(), call.steps()),
    };

    if number_of_subtraces == 0 {
        return None;
    }

    steps.into_iter().rev().find_map(|step| match step {
        MessageTraceStep::Evm(EvmStep { .. }) => None,
        MessageTraceStep::Precompile(precompile) => Some(MessageTrace::Precompile(precompile)),
        MessageTraceStep::Call(call) => Some(MessageTrace::Call(call)),
        MessageTraceStep::Create(create) => Some(MessageTrace::Create(create)),
    })
}

fn get_precompile_message_stack_trace(
    trace: &PrecompileMessageTrace,
) -> Result<Vec<StackTraceEntry>, SolidityTracerError> {
    Ok(vec![StackTraceEntry::PrecompileError {
        precompile: trace.precompile,
    }])
}

fn get_create_message_stack_trace(
    trace: CreateMessageTrace,
) -> Result<Vec<StackTraceEntry>, SolidityTracerError> {
    let inferred_error = error_inferrer::infer_before_tracing_create_message(&trace)?;

    if let Some(inferred_error) = inferred_error {
        return Ok(inferred_error);
    }

    trace_evm_execution(Either::Right(trace))
}

fn get_call_message_stack_trace(
    trace: CallMessageTrace,
) -> Result<Vec<StackTraceEntry>, SolidityTracerError> {
    let inferred_error = error_inferrer::infer_before_tracing_call_message(&trace)?;

    if let Some(inferred_error) = inferred_error {
        return Ok(inferred_error);
    }

    trace_evm_execution(Either::Left(trace))
}

fn get_unrecognized_message_stack_trace(
    trace: Either<CallMessageTrace, CreateMessageTrace>,
) -> Result<Vec<StackTraceEntry>, SolidityTracerError> {
    let (trace_exit_kind, trace_return_data) = match &trace {
        Either::Left(call) => (call.exit(), call.return_data()),
        Either::Right(create) => (create.exit(), create.return_data()),
    };

    let subtrace = get_last_subtrace(&trace);

    if let Some(subtrace) = subtrace {
        let (is_error, return_data) = match &subtrace {
            MessageTrace::Precompile(precompile) => (
                precompile.exit().is_error(),
                precompile.return_data().clone(),
            ),
            MessageTrace::Call(call) => (call.exit().is_error(), call.return_data().clone()),
            MessageTrace::Create(create) => {
                (create.exit().is_error(), create.return_data().clone())
            }
        };

        // This is not a very exact heuristic, but most of the time it will be right, as
        // solidity reverts if a call fails, and most contracts are in
        // solidity
        if is_error && trace_return_data.as_ref() == return_data.as_ref() {
            let unrecognized_entry: StackTraceEntry = match trace {
                Either::Left(CallMessageTrace { address, .. }) => {
                    StackTraceEntry::UnrecognizedContractCallstackEntry {
                        address: address.clone(),
                    }
                }
                Either::Right(CreateMessageTrace { .. }) => {
                    StackTraceEntry::UnrecognizedCreateCallstackEntry
                }
            };

            let mut stacktrace = vec![unrecognized_entry];
            stacktrace.extend(get_stack_trace(subtrace)?);

            return Ok(stacktrace);
        }
    }

    if trace_exit_kind.is_contract_too_large_error() {
        return Ok(vec![StackTraceEntry::ContractTooLargeError {
            source_reference: None,
        }]);
    }

    let is_invalid_opcode_error = trace_exit_kind.is_invalid_opcode_error();

    match trace {
        Either::Left(trace @ CallMessageTrace { .. }) => {
            Ok(vec![StackTraceEntry::UnrecognizedContractError {
                address: trace.address.clone(),
                return_data: trace.return_data().clone(),
                is_invalid_opcode_error,
            }
            .into()])
        }
        Either::Right(trace @ CreateMessageTrace { .. }) => {
            Ok(vec![StackTraceEntry::UnrecognizedCreateError {
                return_data: trace.return_data().clone(),
                is_invalid_opcode_error,
            }
            .into()])
        }
    }
}

fn trace_evm_execution(
    trace: Either<CallMessageTrace, CreateMessageTrace>,
) -> Result<Vec<StackTraceEntry>, SolidityTracerError> {
    let stack_trace = raw_trace_evm_execution(&trace)?;

    if stack_trace_may_require_adjustments(&stack_trace, &trace)? {
        return adjust_stack_trace(stack_trace, &trace).map_err(SolidityTracerError::from);
    }

    Ok(stack_trace)
}

fn raw_trace_evm_execution(
    trace: &Either<CallMessageTrace, CreateMessageTrace>,
) -> Result<Vec<StackTraceEntry>, SolidityTracerError> {
    let (bytecode, steps, number_of_subtraces) = match &trace {
        Either::Left(call) => (call.bytecode(), call.steps(), call.number_of_subtraces()),
        Either::Right(create) => (
            create.bytecode(),
            create.steps(),
            create.number_of_subtraces(),
        ),
    };
    let bytecode = bytecode.as_ref().expect("JS code asserts");

    let mut stacktrace: Vec<StackTraceEntry> = vec![];

    let mut subtraces_seen = 0;

    // There was a jump into a function according to the sourcemaps
    let mut jumped_into_function = false;

    let mut function_jumpdests: Vec<&Instruction> = vec![];

    let mut last_submessage_data: Option<SubmessageData> = None;

    let mut iter = steps.iter().enumerate().peekable();
    while let Some((step_index, step)) = iter.next() {
        if let MessageTraceStep::Evm(EvmStep { pc }) = step {
            let inst = bytecode.get_instruction(*pc)?;

            if inst.jump_type == JumpType::IntoFunction && iter.peek().is_some() {
                let (_, next_step) = iter.peek().unwrap();
                let MessageTraceStep::Evm(next_evm_step) = next_step else {
                    unreachable!("JS code asserted that");
                };
                let next_inst = bytecode.get_instruction(next_evm_step.pc)?;

                if next_inst.opcode == OpCode::JUMPDEST {
                    let frame = instruction_to_callstack_stack_trace_entry(bytecode, inst)?;
                    stacktrace.push(frame);
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
                MessageTraceStep::Evm(_) => unreachable!("branch is taken above"),
                // TODO avoid clones
                MessageTraceStep::Precompile(precompile) => {
                    MessageTrace::Precompile(precompile.clone())
                }
                MessageTraceStep::Call(call) => MessageTrace::Call(call.clone()),
                MessageTraceStep::Create(create) => MessageTrace::Create(create.clone()),
            };

            subtraces_seen += 1;

            // If there are more subtraces, this one didn't terminate the execution
            if subtraces_seen < number_of_subtraces {
                continue;
            }

            let submessage_trace = get_stack_trace(message_trace.clone())?;

            last_submessage_data = Some(SubmessageData {
                message_trace,
                step_index: step_index as u32,
                stacktrace: submessage_trace,
            });
        }
    }

    let stacktrace_with_inferred_error = error_inferrer::infer_after_tracing(
        trace,
        stacktrace,
        &function_jumpdests,
        jumped_into_function,
        last_submessage_data,
    )?;

    error_inferrer::filter_redundant_frames(stacktrace_with_inferred_error)
        .map_err(SolidityTracerError::from)
}
