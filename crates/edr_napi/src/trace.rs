// In contrast to the functions in the `#[napi] impl XYZ` block,
// the free functions `#[napi] pub fn` are exported by napi-rs but
// are considered dead code in the (lib test) target.
// For now, we silence the relevant warnings, as we need to mimick
// the original API while we rewrite the stack trace refinement to Rust.
#![cfg_attr(test, allow(dead_code))]

use edr_chain_spec::EvmHaltReason;
use edr_chain_spec_evm::interpreter::{return_revert, InstructionResult, SuccessOrHalt};
use edr_primitives::{bytecode::opcode::OpCode, Address, U256};
use edr_solidity::{nested_trace::is_calllike_op, tracing::CallTraces};
use edr_solidity_tests::traces::{CallKind, CallTrace, CallTraceArena, CallTraceNode};
use napi::bindgen_prelude::{BigInt, Either, Either3, Uint8Array};
use napi_derive::napi;

use crate::result::{ExceptionalHalt, SuccessReason};

mod library_utils;

mod debug;
mod exit;
mod model;
mod return_data;
pub mod solidity_stack_trace;

/// Matches Hardhat's `MinimalMessage` interface.
#[napi(object)]
pub struct TracingMessage {
    /// Sender address
    #[napi(readonly)]
    pub caller: Uint8Array,

    /// Recipient address. None if it is a Create message.
    #[napi(readonly)]
    pub to: Option<Uint8Array>,

    /// Address of the code that is being executed. Can be different from `to`
    /// if a delegate call is being done.
    #[napi(readonly)]
    pub code_address: Option<Uint8Array>,

    /// Value sent in the message
    #[napi(readonly)]
    pub value: BigInt,

    /// Input data of the message
    #[napi(readonly)]
    pub data: Uint8Array,

    /// Transaction gas limit
    #[napi(readonly)]
    pub gas_limit: BigInt,

    /// Whether it's a static call
    #[napi(readonly)]
    pub is_static_call: bool,
}

/// Matches Hardhat's `MinimalInterpreterStep` interface.
#[napi(object)]
pub struct TracingStep {
    /// The program counter
    #[napi(readonly)]
    pub pc: u32,
    /// Call depth
    #[napi(readonly)]
    pub depth: u32,
    /// The executed opcode
    #[napi(readonly)]
    pub opcode: TracingOpcode,
    /// The entries on the stack.
    #[napi(readonly)]
    pub stack: Vec<BigInt>,
    /// The memory at the step. None if verbose tracing is disabled.
    #[napi(readonly)]
    pub memory: Option<Uint8Array>,
}

/// Opcode information for a tracing step.
#[napi(object)]
pub struct TracingOpcode {
    /// The name of the opcode
    #[napi(readonly)]
    pub name: String,
}

/// Matches Hardhat's `MinimalEVMResult` interface.
#[napi(object)]
pub struct TracingMessageResult {
    /// The execution result
    #[napi(readonly)]
    pub exec_result: TracingExecResult,
}

/// Matches Hardhat's `MinimalExecResult` interface.
#[napi(object)]
pub struct TracingExecResult {
    /// Whether execution succeeded
    #[napi(readonly)]
    pub success: bool,
    /// Gas used during execution
    #[napi(readonly)]
    pub execution_gas_used: BigInt,
    /// Address of the created contract, if any
    #[napi(readonly)]
    pub contract_address: Option<Uint8Array>,
    /// The reason for the exit (success or halt)
    #[napi(readonly)]
    pub reason: Option<Either<SuccessReason, ExceptionalHalt>>,
    /// The output data
    #[napi(readonly)]
    pub output: Option<Uint8Array>,
}

pub(crate) fn u256_to_bigint(v: &edr_primitives::U256) -> BigInt {
    BigInt {
        sign_bit: false,
        words: v.into_limbs().to_vec(),
    }
}

type RawTrace = Either3<TracingMessage, TracingStep, TracingMessageResult>;

/// Converts collected call traces into a flat vector of `RawTrace` messages,
/// following the order that `EthereumJS` would have produced.
///
/// Captured stack tops are consumed in step order during the traversal; they
/// are only materialized as JS values here, so transactions whose traces are
/// never read don't pay for per-step stack snapshots.
pub(crate) fn raw_trace_from_call_trace_arena(
    call_traces: &CallTraces,
    verbose: bool,
) -> Vec<RawTrace> {
    let CallTraces { arena, stack_tops } = call_traces;

    let mut result = Vec::new();
    if let Some(node) = arena.nodes().first() {
        let mut tops = (!stack_tops.is_empty()).then(|| stack_tops.iter());
        convert_node(
            arena,
            node,
            false,
            node.trace.caller,
            verbose,
            &mut tops,
            &mut result,
        );
        debug_assert!(
            tops.as_ref().is_none_or(|tops| tops.len() == 0),
            "captured more stack tops than steps in the arena"
        );
    }
    result
}

/// DFS traversal of the arena, emitting Before/Step/After messages in the flat
/// order that the old `TraceCollector` used to produce.
fn convert_node(
    arena: &CallTraceArena,
    node: &CallTraceNode,
    mut is_static_call: bool,
    original_caller: Address,
    verbose: bool,
    tops: &mut Option<std::slice::Iter<'_, Option<U256>>>,
    output: &mut Vec<Either3<TracingMessage, TracingStep, TracingMessageResult>>,
) {
    let trace = &node.trace;

    // 1. Emit BeforeMessage (TracingMessage)
    let is_create = trace.kind.is_any_create();
    is_static_call |= trace.kind == CallKind::StaticCall;

    output.push(Either3::A(TracingMessage {
        caller: match trace.kind {
            CallKind::DelegateCall => Uint8Array::with_data_copied(original_caller.as_slice()),
            _ => Uint8Array::with_data_copied(trace.caller.as_slice()),
        },
        to: match trace.kind {
            CallKind::Create | CallKind::Create2 => None,
            CallKind::DelegateCall | CallKind::CallCode => {
                Some(Uint8Array::with_data_copied(trace.caller.as_slice()))
            }
            _ => Some(Uint8Array::with_data_copied(trace.address.as_slice())),
        },
        code_address: if is_create {
            None
        } else {
            Some(Uint8Array::with_data_copied(trace.address.as_slice()))
        },
        value: u256_to_bigint(&trace.value),
        data: Uint8Array::with_data_copied(&trace.data),
        gas_limit: BigInt::from(trace.gas_limit),
        is_static_call,
    }));

    // 2. Emit Step messages (TracingStep) and recursively convert child calls
    let mut steps = trace.steps.iter().peekable();
    if let Some(first_step) = steps.peek()
        && first_step.op == OpCode::STOP
    {
        // Historically, Hardhat 2 didn't record the first step if it was a STOP opcode,
        // so we skip it to maintain compatibility with existing traces.
        steps.next();
        // The skipped step still captured a stack top.
        if let Some(tops) = tops.as_mut() {
            tops.next();
        }
    }

    let mut child_index = 0;
    for step in steps {
        output.push(Either3::B(TracingStep {
            pc: step.pc.try_into().expect("PC larger than u32::MAX"),
            depth: trace
                .depth
                .try_into()
                .expect("Call depth larger than u32::MAX"),
            opcode: TracingOpcode {
                name: OpCode::name_by_op(step.op.get()).to_string(),
            },
            stack: match tops.as_mut() {
                // Tops are only captured in configurations where the arena
                // records no stack snapshots, so they take precedence.
                Some(tops) => {
                    let top = tops.next().unwrap_or_else(|| {
                        debug_assert!(false, "more steps in the arena than captured stack tops");
                        &None
                    });
                    top.as_ref()
                        .map(|top| vec![u256_to_bigint(top)])
                        .unwrap_or_default()
                }
                None if verbose => {
                    // Full stack
                    step.stack
                        .as_ref()
                        .map(|s| s.iter().map(u256_to_bigint).collect())
                        .unwrap_or_default()
                }
                None => {
                    // Top of stack only
                    step.stack
                        .as_ref()
                        .and_then(|s| s.last().map(|v| vec![u256_to_bigint(v)]))
                        .unwrap_or_default()
                }
            },
            memory: step.memory.as_ref().and_then(|m| {
                if verbose {
                    Some(Uint8Array::with_data_copied(m.as_bytes()))
                } else {
                    None
                }
            }),
        }));

        if is_calllike_op(step) {
            // The opcode of this step is a call, but it's possible that this step resulted
            // in a revert or out of gas error in which case there's no actual child call executed and recorded: <https://github.com/paradigmxyz/reth/issues/3915>
            if let Some(call_id) = node.children.get(child_index).copied() {
                child_index += 1;

                let child_node = arena
                    .nodes()
                    .get(call_id)
                    .expect("child index should be valid");

                if matches!(step.op, OpCode::CREATE | OpCode::CREATE2)
                    || !should_skip_call(&child_node.trace)
                {
                    convert_node(
                        arena,
                        child_node,
                        is_static_call,
                        match child_node.trace.kind {
                            CallKind::DelegateCall => original_caller,
                            _ => child_node.trace.caller,
                        },
                        verbose,
                        tops,
                        output,
                    );
                }
            }
        }
    }

    // 3. Emit AfterMessage (TracingMessageResult)
    let reason = convert_status(trace.status);

    let contract_address = if is_create && trace.success {
        Some(Uint8Array::with_data_copied(trace.address.as_slice()))
    } else {
        None
    };

    output.push(Either3::C(TracingMessageResult {
        exec_result: TracingExecResult {
            success: trace.success,
            execution_gas_used: BigInt::from(trace.gas_used),
            contract_address,
            reason,
            output: Some(Uint8Array::with_data_copied(&trace.output)),
        },
    }));
}

/// Converts an `InstructionResult` status into an optional reason.
fn convert_status(
    status: Option<InstructionResult>,
) -> Option<Either<SuccessReason, ExceptionalHalt>> {
    let status = status?;

    let success_or_halt: SuccessOrHalt<EvmHaltReason> = status.into();
    match success_or_halt {
        SuccessOrHalt::Success(reason) => Some(Either::A(SuccessReason::from(reason))),
        SuccessOrHalt::Halt(reason) => Some(Either::B(ExceptionalHalt::from(reason))),
        SuccessOrHalt::Revert => None,
        SuccessOrHalt::FatalExternalError => {
            panic!("A `FatalExternalError` should not be included in a `CallTraceArena`.")
        }
        SuccessOrHalt::Internal(error) => {
            panic!("An `Internal` error should not be included in a `CallTraceArena`: {error:?}")
        }
    }
}

// Historically, Hardhat 2 didn't emit a before message for calls that failed
// immediately with a revert or out of gas error, so we skip them to maintain
// compatibility with existing traces.
fn should_skip_call(trace: &CallTrace) -> bool {
    trace.steps.is_empty()
        && trace
            .status
            .is_some_and(|status| matches!(status, return_revert!()))
}

#[cfg(test)]
mod tests {
    use edr_primitives::Bytes;
    use edr_solidity_tests::traces::CallTraceStep;

    use super::*;

    fn step(op: OpCode) -> CallTraceStep {
        CallTraceStep {
            pc: 0,
            op,
            stack: None,
            push_stack: None,
            memory: None,
            returndata: Bytes::new(),
            gas_remaining: 0,
            gas_refund_counter: 0,
            gas_used: 0,
            gas_cost: 0,
            storage_change: None,
            status: None,
            immediate_bytes: None,
            decoded: None,
        }
    }

    /// Root with three steps, the second of which calls into a child frame
    /// with two steps: execution order is root step 0, root step 1 (`CALL`),
    /// child steps 0-1, root step 2.
    fn nested_arena() -> CallTraceArena {
        let mut arena = CallTraceArena::default();

        let root = &mut arena.nodes_mut()[0];
        root.children = vec![1];
        root.trace.steps = vec![step(OpCode::PUSH1), step(OpCode::CALL), step(OpCode::PUSH1)];

        let mut child = CallTraceNode {
            parent: Some(0),
            idx: 1,
            ..CallTraceNode::default()
        };
        child.trace.depth = 1;
        child.trace.steps = vec![step(OpCode::PUSH1), step(OpCode::PUSH1)];
        arena.nodes_mut().push(child);

        arena
    }

    /// Extracts the emitted steps' stacks as `u64` values.
    fn emitted_stacks(output: &[RawTrace]) -> Vec<Vec<u64>> {
        output
            .iter()
            .filter_map(|item| match item {
                Either3::B(step) => Some(
                    step.stack
                        .iter()
                        .map(|value| {
                            assert!(!value.sign_bit);
                            value.words[0]
                        })
                        .collect(),
                ),
                _ => None,
            })
            .collect()
    }

    fn tops(values: &[Option<u64>]) -> Vec<Option<U256>> {
        values.iter().map(|top| top.map(U256::from)).collect()
    }

    #[test]
    fn stack_tops_follow_execution_order_across_frames() {
        let call_traces = CallTraces {
            arena: nested_arena(),
            stack_tops: tops(&[Some(10), Some(11), None, Some(20), Some(12)]),
        };

        let output = raw_trace_from_call_trace_arena(&call_traces, false);

        assert_eq!(
            emitted_stacks(&output),
            [
                vec![10], // root step 0
                vec![11], // root step 1 (CALL)
                vec![],   // child step 0: empty stack
                vec![20], // child step 1
                vec![12], // root step 2
            ]
        );
    }

    #[test]
    fn skipped_leading_stop_step_still_consumes_a_stack_top() {
        let mut arena = CallTraceArena::default();
        arena.nodes_mut()[0].trace.steps = vec![step(OpCode::STOP), step(OpCode::PUSH1)];

        let call_traces = CallTraces {
            arena,
            stack_tops: tops(&[Some(1), Some(2)]),
        };

        let output = raw_trace_from_call_trace_arena(&call_traces, false);

        assert_eq!(emitted_stacks(&output), [vec![2]]);
    }

    #[test]
    fn without_captured_tops_recorded_stacks_are_used() {
        let mut arena = CallTraceArena::default();
        let recorded = [U256::from(1), U256::from(2)];
        arena.nodes_mut()[0].trace.steps = vec![step(OpCode::PUSH1)];
        arena.nodes_mut()[0].trace.steps[0].stack = Some(Box::from(recorded));

        let call_traces = CallTraces {
            arena,
            stack_tops: Vec::new(),
        };

        let verbose = raw_trace_from_call_trace_arena(&call_traces, true);
        assert_eq!(emitted_stacks(&verbose), [vec![1, 2]]);

        let non_verbose = raw_trace_from_call_trace_arena(&call_traces, false);
        assert_eq!(emitted_stacks(&non_verbose), [vec![2]]);
    }
}
