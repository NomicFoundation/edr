//! Conversion from `CallTraceArena` to provider's `Trace` format.

use edr_chain_spec::HaltReasonTrait;
use edr_chain_spec_evm::{
    interpreter::{InstructionResult, SuccessOrHalt},
    result::{ExecutionResult, Output, SuccessReason},
};
use edr_primitives::{Bytecode, Bytes};
use revm_inspectors::tracing::{types::CallTraceStep, CallTraceArena};

use crate::{AfterMessage, BeforeMessage, Stack, Step, Trace, TraceMessage};

/// Converts a `CallTraceArena` to the provider's `Trace` format.
///
/// This function flattens the hierarchical trace arena into a sequential
/// list of trace messages (Before/Step/After).
pub fn convert_arena_to_trace<HaltReasonT: HaltReasonTrait>(
    arena: &CallTraceArena,
) -> Trace<HaltReasonT> {
    let mut messages = Vec::new();

    if !arena.nodes().is_empty() {
        convert_node(arena, 0, &mut messages, false);
    }

    // The return value is the output of the root trace
    let return_value = arena
        .nodes()
        .first()
        .map(|node| node.trace.output.clone())
        .unwrap_or_default();

    Trace {
        messages,
        return_value,
    }
}

fn convert_node<HaltReasonT: HaltReasonTrait>(
    arena: &CallTraceArena,
    node_idx: usize,
    messages: &mut Vec<TraceMessage<HaltReasonT>>,
    verbose: bool,
) {
    let nodes = arena.nodes();
    let node = &nodes[node_idx];
    let trace = &node.trace;

    // Emit Before message
    emit_before_message(&node.trace, &node.children, messages);

    // Process steps and child traces
    let mut child_index = 0;
    for step in &trace.steps {
        if is_calllike_op(step) {
            // Check if there's a corresponding child call
            if let Some(&call_id) = node.children.get(child_index) {
                child_index += 1;
                // Recursively process the child
                convert_node(arena, call_id, messages, verbose);
            }
        } else {
            // Emit a Step message for non-call opcodes
            emit_step_message(step, trace.depth, messages, verbose);
        }
    }

    // Emit After message
    emit_after_message(&node.trace, messages);
}

fn emit_before_message<HaltReasonT: HaltReasonTrait>(
    trace: &revm_inspectors::tracing::types::CallTrace,
    _children: &[usize],
    messages: &mut Vec<TraceMessage<HaltReasonT>>,
) {
    let (to, code, code_address) = if trace.kind.is_any_create() {
        // For create, there's no target address initially
        (None, None, None)
    } else {
        // For calls, include the target address and code
        let code = if trace.data.is_empty() {
            None
        } else {
            // Convert bytes to bytecode - this is a simplified version
            // In reality, the bytecode analysis might be more complex
            Some(Bytecode::new_raw(trace.data.clone()))
        };

        (Some(trace.address), code, Some(trace.address))
    };

    let message = BeforeMessage {
        depth: trace.depth,
        caller: trace.caller,
        to,
        is_static_call: trace.kind.is_static_call(),
        gas_limit: trace.gas_used, // This is the gas used, not limit - arena doesn't track limit
        data: trace.data.clone(),
        value: trace.value,
        code_address,
        code,
    };

    messages.push(TraceMessage::Before(message));
}

fn emit_step_message<HaltReasonT: HaltReasonTrait>(
    step: &CallTraceStep,
    depth: usize,
    messages: &mut Vec<TraceMessage<HaltReasonT>>,
    verbose: bool,
) {
    let stack = if verbose {
        // In verbose mode, we'd need the full stack, but CallTraceStep doesn't have it
        // For now, just use Top
        Stack::Top(None)
    } else {
        Stack::Top(None)
    };

    let step_msg = Step {
        pc: step.pc as u32,
        depth: depth as u64,
        opcode: step.op.get(),
        stack,
        memory: None, // CallTraceStep doesn't include memory
    };

    messages.push(TraceMessage::Step(step_msg));
}

fn emit_after_message<HaltReasonT: HaltReasonTrait>(
    trace: &revm_inspectors::tracing::types::CallTrace,
    messages: &mut Vec<TraceMessage<HaltReasonT>>,
) {
    let execution_result = convert_instruction_result(
        trace.status,
        trace.gas_used,
        trace.output.clone(),
        trace.address,
        trace.kind.is_any_create(),
    );

    let contract_address = if trace.kind.is_any_create() {
        // For successful creates, the address is the deployed contract
        match &execution_result {
            ExecutionResult::Success { .. } => Some(trace.address),
            _ => None,
        }
    } else {
        None
    };

    let message = AfterMessage {
        execution_result,
        contract_address,
    };

    messages.push(TraceMessage::After(message));
}

fn convert_instruction_result<HaltReasonT: HaltReasonTrait>(
    result: Option<InstructionResult>,
    gas_used: u64,
    output: Bytes,
    address: edr_primitives::Address,
    is_create: bool,
) -> ExecutionResult<HaltReasonT> {
    let Some(result) = result else {
        // If there's no result, treat it as success with zero gas
        return ExecutionResult::Success {
            reason: SuccessReason::Return,
            gas_used: 0,
            gas_refunded: 0,
            logs: Vec::new(),
            output: if is_create {
                Output::Create(output, Some(address))
            } else {
                Output::Call(output)
            },
        };
    };

    let success_or_halt: SuccessOrHalt<HaltReasonT> = result.into();

    match success_or_halt {
        SuccessOrHalt::Success(reason) => ExecutionResult::Success {
            reason,
            gas_used,
            gas_refunded: 0,  // Arena doesn't track refunded gas
            logs: Vec::new(), // Arena doesn't track logs per call
            output: if is_create {
                Output::Create(output, Some(address))
            } else {
                Output::Call(output)
            },
        },
        SuccessOrHalt::Revert => ExecutionResult::Revert { gas_used, output },
        SuccessOrHalt::Halt(reason) => ExecutionResult::Halt { reason, gas_used },
        SuccessOrHalt::Internal(_) => {
            // Internal errors shouldn't occur in normal execution
            ExecutionResult::Revert {
                gas_used,
                output: Bytes::new(),
            }
        }
        SuccessOrHalt::FatalExternalError => {
            // Fatal errors shouldn't occur in normal execution
            ExecutionResult::Revert {
                gas_used,
                output: Bytes::new(),
            }
        }
    }
}

fn is_calllike_op(step: &CallTraceStep) -> bool {
    use edr_primitives::bytecode::opcode;

    matches!(
        step.op.get(),
        opcode::CALL
            | opcode::DELEGATECALL
            | opcode::STATICCALL
            | opcode::CREATE
            | opcode::CALLCODE
            | opcode::CREATE2
    )
}
