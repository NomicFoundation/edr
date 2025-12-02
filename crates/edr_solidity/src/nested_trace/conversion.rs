//! Conversion from `CallTraceArena` (from revm-inspectors) to `NestedTrace`.

use std::collections::HashMap;

use edr_chain_spec::HaltReasonTrait;
use edr_primitives::{Address, Bytes, U160};
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
    let node = arena
        .nodes()
        .get(node_idx)
        .expect("node index should be valid");
    let trace = &node.trace;

    // Based on https://github.com/paradigmxyz/revm-inspectors/blob/ceef3f3624ca51bf3c41c97d6c013606db3a6019/src/tracing/types.rs#L257
    let mut steps = Vec::new();
    let mut child_index = 0;
    for step in &trace.steps {
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
                steps.push(match child_trace {
                    NestedTrace::Create(msg) => NestedTraceStep::Create(msg),
                    NestedTrace::Call(msg) => NestedTraceStep::Call(msg),
                    NestedTrace::Precompile(msg) => NestedTraceStep::Precompile(msg),
                });
            }
        } else {
            steps.push(NestedTraceStep::Evm(EvmStep { pc: step.pc as u32 }));
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
