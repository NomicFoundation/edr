use std::collections::HashMap;

use alloy_primitives::{Address, Bytes};
use edr_solidity::{
    contract_decoder::ContractDecoder,
    exit_code::ExitCode,
    nested_trace::{
        CallMessage, CreateMessage, EvmStep, NestedTrace, NestedTraceStep, PrecompileMessage,
    },
    solidity_stack_trace::StackTraceEntry,
    solidity_tracer::{self, SolidityTracerError},
};
use foundry_evm::traces::{CallTraceArena, CallTraceStep, TraceKind};

use crate::{
    constants::{CHEATCODE_ADDRESS, HARDHAT_CONSOLE_ADDRESS},
    revm::primitives::ruint::aliases::U160,
};

#[derive(Debug, thiserror::Error)]
pub enum StackTraceError {
    #[error("Invalid root node in call trace arena")]
    InvalidRootNode,
    #[error(transparent)]
    Tracer(#[from] SolidityTracerError),
}

pub fn get_stack_trace(
    contract_decoder: &ContractDecoder,
    traces: &[(TraceKind, CallTraceArena)],
) -> Result<Option<Vec<StackTraceEntry>>, StackTraceError> {
    let mut address_to_creation_code = HashMap::new();
    let mut address_to_runtime_code = HashMap::new();

    for arena in traces.into_iter().map(|(_, trace)| trace) {
        for node in arena.nodes() {
            let address = node.trace.address;
            if node.trace.kind.is_any_create() {
                address_to_creation_code.insert(address, &node.trace.data);
                address_to_runtime_code.insert(address, &node.trace.output);
            }
        }
    }

    for (kind, trace) in traces {
        if kind.is_error() {
            let trace = convert_call_trace_arena_to_nested_trace(
                &address_to_creation_code,
                &address_to_runtime_code,
                trace,
            )?;
            let trace = contract_decoder.try_to_decode_message_trace(trace);
            let stack_trace = solidity_tracer::get_stack_trace(trace)?;
            return Ok(Some(stack_trace));
        }
    }
    Ok(None)
}

fn convert_call_trace_arena_to_nested_trace(
    address_to_creation_code: &HashMap<Address, &Bytes>,
    address_to_runtime_code: &HashMap<Address, &Bytes>,
    arena: &CallTraceArena,
) -> Result<NestedTrace, StackTraceError> {
    // Start conversion from the root node (index 0)
    if arena.nodes().is_empty() {
        return Err(StackTraceError::InvalidRootNode);
    }

    convert_node_to_nested_trace(address_to_creation_code, address_to_runtime_code, &arena, 0)
}

fn convert_node_to_nested_trace(
    address_to_creation_code: &HashMap<Address, &Bytes>,
    address_to_runtime_code: &HashMap<Address, &Bytes>,
    arena: &CallTraceArena,
    node_idx: usize,
) -> Result<NestedTrace, StackTraceError> {
    let node = &arena.nodes()[node_idx];
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
                let child_trace = convert_node_to_nested_trace(
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

    // Convert based on call type and precompile status
    if node.is_precompile() {
        let precompile: U160 = trace.address.into();
        let precompile: u32 = precompile
            .try_into()
            .expect("MAX_PRECOMPILE_NUMBER is of type u16 so it fits");
        Ok(NestedTrace::Precompile(PrecompileMessage {
            precompile,
            calldata: trace.data.clone(),
            value: trace.value,
            return_data: trace.output.clone(),
            exit: convert_instruction_result_to_exit_code(trace.status),
            gas_used: trace.gas_used,
            depth: trace.depth,
        }))
    } else if trace.kind.is_any_create() {
        Ok(NestedTrace::Create(CreateMessage {
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
        }))
    } else {
        let code = if trace.address == HARDHAT_CONSOLE_ADDRESS || trace.address == CHEATCODE_ADDRESS
        {
            // HACK: use address as code if the library is implemented in Rust
            // TODO: how should we handle contract metadata?
            Bytes::from(trace.address.to_vec())
        } else {
            address_to_runtime_code
                .get(&trace.address)
                .map(|c| (*c).clone())
                // Code might not exist if it's a mocked contract
                // Mimicking behavior here: https://github.com/NomicFoundation/edr/blob/4e7491d8631da27b4bd1ba2bde4914bb704e2c52/crates/foundry/cheatcodes/src/evm/mock.rs#L75
                .unwrap_or_else(|| Bytes::from_static(&[0u8]))
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
}

// EDR uses REVM 12 in this branch, but Foundry is on REVM 13.
// We can't bump EDR to REVM 13, because it needs the `hashbrown` feature of
// `revm-primitives`, but enabling that feature is incompatible with
// `foundry-fork-db`.
fn convert_halt_reason(
    halt: revm_foundry::primitives::HaltReason,
) -> revm_edr::primitives::HaltReason {
    use revm_foundry::primitives::HaltReason;
    match halt {
        HaltReason::OutOfGas(err) => {
            revm_edr::primitives::HaltReason::OutOfGas(convert_out_of_gas_error(err))
        }
        HaltReason::OpcodeNotFound => revm_edr::primitives::HaltReason::OpcodeNotFound,
        HaltReason::InvalidFEOpcode => revm_edr::primitives::HaltReason::InvalidFEOpcode,
        HaltReason::InvalidJump => revm_edr::primitives::HaltReason::InvalidJump,
        HaltReason::NotActivated => revm_edr::primitives::HaltReason::NotActivated,
        HaltReason::StackOverflow => revm_edr::primitives::HaltReason::StackOverflow,
        HaltReason::StackUnderflow => revm_edr::primitives::HaltReason::StackUnderflow,
        HaltReason::OutOfOffset => revm_edr::primitives::HaltReason::OutOfOffset,
        HaltReason::CreateCollision => revm_edr::primitives::HaltReason::CreateCollision,
        HaltReason::PrecompileError => revm_edr::primitives::HaltReason::PrecompileError,
        HaltReason::NonceOverflow => revm_edr::primitives::HaltReason::NonceOverflow,
        HaltReason::CreateContractSizeLimit => {
            revm_edr::primitives::HaltReason::CreateContractSizeLimit
        }
        HaltReason::CreateContractStartingWithEF => {
            revm_edr::primitives::HaltReason::CreateContractStartingWithEF
        }
        HaltReason::CreateInitCodeSizeLimit => {
            revm_edr::primitives::HaltReason::CreateInitCodeSizeLimit
        }
        HaltReason::OverflowPayment => revm_edr::primitives::HaltReason::OverflowPayment,
        HaltReason::StateChangeDuringStaticCall => {
            revm_edr::primitives::HaltReason::StateChangeDuringStaticCall
        }
        HaltReason::CallNotAllowedInsideStatic => {
            revm_edr::primitives::HaltReason::CallNotAllowedInsideStatic
        }
        HaltReason::OutOfFunds => revm_edr::primitives::HaltReason::OutOfFunds,
        HaltReason::CallTooDeep => revm_edr::primitives::HaltReason::CallTooDeep,
        HaltReason::EofAuxDataOverflow => revm_edr::primitives::HaltReason::EofAuxDataOverflow,
        HaltReason::EofAuxDataTooSmall => revm_edr::primitives::HaltReason::EofAuxDataTooSmall,
        HaltReason::EOFFunctionStackOverflow => {
            revm_edr::primitives::HaltReason::EOFFunctionStackOverflow
        }
        // TODO discuss: this was added in REVM 13: https://github.com/bluealloy/revm/pull/1570
        // This seems to be the closest error:
        HaltReason::InvalidEXTCALLTarget => revm_edr::primitives::HaltReason::EofAuxDataTooSmall,
        // TODO discuss: this is optimism only,but enabled the `optimism` feature for EDR REVM
        // causes compilation errors
        HaltReason::FailedDeposit => revm_edr::primitives::HaltReason::NotActivated,
    }
}

fn convert_out_of_gas_error(
    err: revm_foundry::primitives::OutOfGasError,
) -> revm_edr::primitives::OutOfGasError {
    use revm_foundry::primitives::OutOfGasError;
    match err {
        OutOfGasError::Basic => revm_edr::primitives::OutOfGasError::Basic,
        OutOfGasError::Memory => revm_edr::primitives::OutOfGasError::Memory,
        OutOfGasError::MemoryLimit => revm_edr::primitives::OutOfGasError::MemoryLimit,
        OutOfGasError::Precompile => revm_edr::primitives::OutOfGasError::Precompile,
        OutOfGasError::InvalidOperand => revm_edr::primitives::OutOfGasError::InvalidOperand,
    }
}

fn convert_instruction_result_to_exit_code(
    result: revm_foundry::interpreter::InstructionResult,
) -> ExitCode {
    let success_or_halt: revm_foundry::interpreter::SuccessOrHalt = result.into();
    if success_or_halt.is_success() {
        ExitCode::Success
    } else if success_or_halt.is_revert() {
        ExitCode::Revert
    } else {
        let halt = success_or_halt.to_halt().expect("must be a halt");
        ExitCode::Halt(convert_halt_reason(halt))
    }
}

fn is_calllike_op(step: &CallTraceStep) -> bool {
    use revm_foundry::interpreter::opcode;

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
