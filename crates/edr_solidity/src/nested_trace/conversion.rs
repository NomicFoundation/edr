//! Conversion from `CallTraceArena` (from revm-inspectors) to `NestedTrace`.

use edr_chain_spec::{EvmHaltReason, HaltReasonTrait};
use edr_primitives::{Address, Bytes, HashMap, U160};
use revm_inspectors::tracing::{types::CallTraceStep, CallTraceArena};
use revm_interpreter::{InternalResult, SuccessOrHalt};

use super::{CallMessage, CreateMessage, EvmStep, NestedTrace, NestedTraceStep, PrecompileMessage};
use crate::exit_code::ExitCode;

/// Error type for converting `CallTraceArena` to `NestedTrace`.
#[derive(Clone, Debug, thiserror::Error)]
pub enum CallTraceArenaConversionError {
    /// Invalid root node in call trace arena
    #[error("Invalid root node in call trace arena")]
    InvalidRootNode,
}

/// Converts a `CallTraceArena` into a `NestedTrace`.
pub(super) fn convert_from_arena<HaltReasonT: HaltReasonTrait>(
    address_to_creation_code: &HashMap<Address, &Bytes>,
    address_to_runtime_code: &HashMap<Address, &Bytes>,
    arena: &CallTraceArena,
) -> Result<NestedTrace<HaltReasonT>, CallTraceArenaConversionError> {
    // Start conversion from the root node (index 0)
    if arena.nodes().is_empty() {
        return Err(CallTraceArenaConversionError::InvalidRootNode);
    }

    convert_node(address_to_creation_code, address_to_runtime_code, arena, 0)
}

fn convert_node<HaltReasonT: HaltReasonTrait>(
    address_to_creation_code: &HashMap<Address, &Bytes>,
    address_to_runtime_code: &HashMap<Address, &Bytes>,
    arena: &CallTraceArena,
    node_idx: usize,
) -> Result<NestedTrace<HaltReasonT>, CallTraceArenaConversionError> {
    // Handle regular calls
    // HACK: use address as code for contracts implemented in Rust
    // (console/cheatcodes)
    const CHEATCODE_ADDRESS: Address = Address::new([
        0x71, 0x09, 0x70, 0x9E, 0xcf, 0xa9, 0x1a, 0x80, 0x62, 0x6f, 0xf3, 0x98, 0x9d, 0x68, 0xf6,
        0x7f, 0x5b, 0x1d, 0xd1, 0x2d,
    ]);
    const HARDHAT_CONSOLE_ADDRESS: Address = Address::new([
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x01,
    ]);

    let node = arena
        .nodes()
        .get(node_idx)
        .expect("node index should be valid");
    let trace = &node.trace;

    // Based on https://github.com/paradigmxyz/revm-inspectors/blob/ceef3f3624ca51bf3c41c97d6c013606db3a6019/src/tracing/types.rs#L257
    let mut steps = Vec::new();
    let mut child_index = 0;
    for step in &trace.steps {
        steps.push(NestedTraceStep::Evm(EvmStep { pc: step.pc as u32 }));

        if is_calllike_op(step) {
            // The opcode of this step is a call, but it's possible that this step resulted
            // in a revert or out of gas error in which case there's no actual child call executed and recorded: <https://github.com/paradigmxyz/reth/issues/3915>
            if let Some(call_id) = node.children.get(child_index).copied() {
                child_index += 1;
                let child_trace = convert_node(
                    address_to_creation_code,
                    address_to_runtime_code,
                    arena,
                    call_id,
                )?;

                // To ensure the Solidity stack trace heuristics work correctly, we don't add
                // failed calls to the nested trace steps.
                if !call_opcode_failed(&child_trace) {
                    steps.push(match child_trace {
                        NestedTrace::Create(msg) => NestedTraceStep::Create(msg),
                        NestedTrace::Call(msg) => NestedTraceStep::Call(msg),
                        NestedTrace::Precompile(msg) => NestedTraceStep::Precompile(msg),
                    });
                }
            }
        }
    }

    // Handle precompile calls
    if node.is_precompile() {
        let precompile: U160 = trace.address.into();
        let precompile: u32 = precompile
            .try_into()
            .expect("MAX_PRECOMPILE_NUMBER is of type u16 so it fits");
        return Ok(NestedTrace::Precompile(PrecompileMessage {
            precompile,
            calldata: trace.data.clone(),
            value: trace.value,
            return_data: trace.output.clone(),
            exit: convert_instruction_result_to_exit_code(trace.status),
            gas_used: trace.gas_used,
            depth: trace.depth,
        }));
    }

    // Handle create calls
    if trace.kind.is_any_create() {
        return Ok(NestedTrace::Create(CreateMessage {
            number_of_subtraces: node.children.len() as u32,
            steps,
            contract_meta: None, // This will be populated by the nested trace decoder
            deployed_contract: Some(trace.output.clone()),
            code: address_to_creation_code
                .get(&trace.address)
                .map(|c| (*c).clone())
                .expect("Create must have code"),
            value: trace.value,
            return_data: trace.output.clone(),
            exit: convert_instruction_result_to_exit_code(trace.status),
            gas_used: trace.gas_used,
            depth: trace.depth,
        }));
    }

    let code = if trace.address == HARDHAT_CONSOLE_ADDRESS || trace.address == CHEATCODE_ADDRESS {
        Bytes::from(trace.address.to_vec())
    } else {
        address_to_runtime_code
            .get(&trace.address)
            // Code might not exist if it's a mocked contract
            .map_or_else(|| Bytes::from_static(&[0u8]), |c| (*c).clone())
    };

    Ok(NestedTrace::Call(CallMessage {
        number_of_subtraces: node.children.len() as u32,
        steps,
        contract_meta: None, // This will be populated by the nested trace decoder
        calldata: trace.data.clone(),
        address: trace.address,
        code_address: trace.address,
        code,
        value: trace.value,
        return_data: trace.output.clone(),
        exit: convert_instruction_result_to_exit_code(trace.status),
        gas_used: trace.gas_used,
        depth: trace.depth,
    }))
}

/// Checks whether a call opcode failed.
///
/// This would be the case if:
/// 1. the procedural macro [`edr_chain_spec_evm::interpreter::return_revert`]
///    is true
/// 2. no steps occurred after the call instruction (i.e., the call failed
///    immediately)
fn call_opcode_failed<HaltReasonT: HaltReasonTrait>(step: &NestedTrace<HaltReasonT>) -> bool {
    match step {
        NestedTrace::Create(create_message) => {
            create_message.steps.is_empty() && is_return_revert_exit_code(&create_message.exit)
        }
        NestedTrace::Call(call_message) => {
            call_message.steps.is_empty() && is_return_revert_exit_code(&call_message.exit)
        }
        NestedTrace::Precompile(precompile_message) => {
            is_return_revert_exit_code(&precompile_message.exit)
        }
    }
}

/// Checks whether the given exit code corresponds to a revert instruction
/// results, matching [`edr_chain_spec_evm::interpreter::return_revert`].
fn is_return_revert_exit_code<HaltReasonT: HaltReasonTrait>(
    exit_code: &ExitCode<HaltReasonT>,
) -> bool {
    matches!(
        exit_code,
        ExitCode::Revert | ExitCode::InvalidExtDelegateCallTarget
    ) || matches!(exit_code, ExitCode::Halt(halt_reason) if *halt_reason == EvmHaltReason::CallTooDeep.into() || *halt_reason == EvmHaltReason::OutOfFunds.into())
}

fn convert_instruction_result_to_exit_code<HaltReasonT: HaltReasonTrait>(
    result: Option<revm_interpreter::InstructionResult>,
) -> ExitCode<HaltReasonT> {
    let Some(result) = result else {
        return ExitCode::InternalContinue;
    };
    let success_or_halt: SuccessOrHalt<HaltReasonT> = result.into();
    match success_or_halt {
        SuccessOrHalt::Success(_) => ExitCode::Success,
        SuccessOrHalt::Revert => ExitCode::Revert,
        SuccessOrHalt::Halt(halt) => ExitCode::Halt(halt),
        SuccessOrHalt::FatalExternalError => ExitCode::FatalExternalError,
        SuccessOrHalt::Internal(result) => match result {
            InternalResult::CreateInitCodeStartingEF00 => ExitCode::CreateInitCodeStartingEF00,
            InternalResult::InvalidExtDelegateCallTarget => ExitCode::InvalidExtDelegateCallTarget,
        },
    }
}

fn is_calllike_op(step: &CallTraceStep) -> bool {
    use revm_bytecode::opcode;

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

/// Control-flow axis tests for [`convert_node`]. Each test isolates one
/// dispatch branch (CALL / CREATE / PRECOMPILE / cheatcode / console.log)
/// against a hand-built `CallTraceArena`, so a regression in any branch
/// surfaces without needing a full provider round-trip.
#[cfg(test)]
mod tests {
    use edr_chain_spec::EvmHaltReason;
    use edr_primitives::Bytes;
    use revm_inspectors::tracing::types::{CallKind, CallTrace, CallTraceNode};

    use super::*;

    /// Cheatcode HEVM address used by foundry-style cheatcodes.
    const CHEATCODE_ADDRESS: Address = Address::new([
        0x71, 0x09, 0x70, 0x9E, 0xcf, 0xa9, 0x1a, 0x80, 0x62, 0x6f, 0xf3, 0x98, 0x9d, 0x68, 0xf6,
        0x7f, 0x5b, 0x1d, 0xd1, 0x2d,
    ]);
    const HARDHAT_CONSOLE_ADDRESS: Address = Address::new([
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x01,
    ]);

    fn arena_with_root(trace: CallTrace) -> CallTraceArena {
        let mut arena = CallTraceArena::default();
        arena.nodes_mut()[0] = CallTraceNode {
            trace,
            ..Default::default()
        };
        arena
    }

    #[test]
    fn empty_arena_returns_invalid_root_node_error() {
        let mut arena = CallTraceArena::default();
        arena.nodes_mut().clear();
        let err =
            convert_from_arena::<EvmHaltReason>(&HashMap::default(), &HashMap::default(), &arena)
                .expect_err("empty arena must fail");
        assert!(matches!(
            err,
            CallTraceArenaConversionError::InvalidRootNode
        ));
    }

    #[test]
    fn regular_call_produces_call_variant_with_runtime_code() {
        let addr = Address::new([0xaa; 20]);
        let runtime_bytes = Bytes::from_static(&[0x60, 0x80, 0x60, 0x40]);
        let arena = arena_with_root(CallTrace {
            kind: CallKind::Call,
            address: addr,
            ..Default::default()
        });
        let mut runtime: HashMap<Address, &Bytes> = HashMap::default();
        runtime.insert(addr, &runtime_bytes);

        let trace = convert_from_arena::<EvmHaltReason>(&HashMap::default(), &runtime, &arena)
            .expect("conversion must succeed");

        match trace {
            NestedTrace::Call(msg) => {
                assert_eq!(msg.address, addr);
                assert_eq!(msg.code, runtime_bytes, "code must come from runtime map");
            }
            other => panic!("expected NestedTrace::Call, got {other:?}"),
        }
    }

    #[test]
    fn create_node_produces_create_variant() {
        let addr = Address::new([0xbb; 20]);
        let init_code = Bytes::from_static(&[0xfe, 0xed, 0xfa, 0xce]);
        let arena = arena_with_root(CallTrace {
            kind: CallKind::Create,
            address: addr,
            ..Default::default()
        });
        let mut creation: HashMap<Address, &Bytes> = HashMap::default();
        creation.insert(addr, &init_code);

        let trace = convert_from_arena::<EvmHaltReason>(&creation, &HashMap::default(), &arena)
            .expect("conversion must succeed");

        match trace {
            NestedTrace::Create(msg) => {
                assert_eq!(
                    msg.code, init_code,
                    "create code must come from creation map"
                );
            }
            other => panic!("expected NestedTrace::Create, got {other:?}"),
        }
    }

    #[test]
    fn precompile_node_produces_precompile_variant() {
        // Precompile #1 (ECRECOVER) — address 0x...01, maybe_precompile = Some(true).
        let precompile_addr =
            Address::new([0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1]);
        let mut arena = CallTraceArena::default();
        arena.nodes_mut()[0] = CallTraceNode {
            trace: CallTrace {
                kind: CallKind::Call,
                address: precompile_addr,
                maybe_precompile: Some(true),
                ..Default::default()
            },
            ..Default::default()
        };

        let trace =
            convert_from_arena::<EvmHaltReason>(&HashMap::default(), &HashMap::default(), &arena)
                .expect("conversion must succeed");

        match trace {
            NestedTrace::Precompile(msg) => {
                assert_eq!(msg.precompile, 1, "ECRECOVER precompile number is 1");
            }
            other => panic!("expected NestedTrace::Precompile, got {other:?}"),
        }
    }

    #[test]
    fn cheatcode_address_uses_address_as_code() {
        let arena = arena_with_root(CallTrace {
            kind: CallKind::Call,
            address: CHEATCODE_ADDRESS,
            ..Default::default()
        });

        let trace =
            convert_from_arena::<EvmHaltReason>(&HashMap::default(), &HashMap::default(), &arena)
                .expect("conversion must succeed");

        match trace {
            NestedTrace::Call(msg) => {
                assert_eq!(
                    msg.code,
                    Bytes::from(CHEATCODE_ADDRESS.to_vec()),
                    "cheatcode call's `code` must be its address bytes"
                );
            }
            other => panic!("expected NestedTrace::Call, got {other:?}"),
        }
    }

    #[test]
    fn console_log_address_uses_address_as_code() {
        let arena = arena_with_root(CallTrace {
            kind: CallKind::Call,
            address: HARDHAT_CONSOLE_ADDRESS,
            ..Default::default()
        });

        let trace =
            convert_from_arena::<EvmHaltReason>(&HashMap::default(), &HashMap::default(), &arena)
                .expect("conversion must succeed");

        match trace {
            NestedTrace::Call(msg) => {
                assert_eq!(
                    msg.code,
                    Bytes::from(HARDHAT_CONSOLE_ADDRESS.to_vec()),
                    "console.log call's `code` must be its address bytes"
                );
            }
            other => panic!("expected NestedTrace::Call, got {other:?}"),
        }
    }
}
