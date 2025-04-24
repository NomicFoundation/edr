use std::{borrow::Cow, collections::HashMap};

use alloy_primitives::{Address, Bytes, U160};
use edr_solidity::{
    contract_decoder::{ContractDecoderError, NestedTraceDecoder},
    exit_code::ExitCode,
    nested_trace::{
        CallMessage, CreateMessage, EvmStep, NestedTrace, NestedTraceStep, PrecompileMessage,
    },
    solidity_stack_trace::StackTraceEntry,
    solidity_tracer::{self, SolidityTracerError},
};
use foundry_evm_core::{
    backend::IndeterminismReasons,
    constants::{CHEATCODE_ADDRESS, HARDHAT_CONSOLE_ADDRESS},
};
use foundry_evm_traces::{SparsedTraceArena, TraceKind};
use revm_inspectors::tracing::{types::CallTraceStep, CallTraceArena};

use crate::executors::EvmError;

/// Stack trace generation error during re-execution.
#[derive(Clone, Debug, thiserror::Error)]
pub enum StackTraceError {
    #[error(transparent)]
    ContractDecoder(#[from] ContractDecoderError),
    #[error("Unexpected EVM execution error: {0}")]
    Evm(String),
    #[error("Test setup unexpectedly failed during execution with revert reason: {0}")]
    FailingSetup(String),
    #[error("Invalid root node in call trace arena")]
    InvalidRootNode,
    #[error(transparent)]
    Tracer(#[from] SolidityTracerError),
}

// `EvmError` is not `Clone`
impl From<EvmError> for StackTraceError {
    fn from(value: EvmError) -> Self {
        Self::Evm(value.to_string())
    }
}

/// Compute stack trace based on execution traces.
/// Assumes last trace is the error one. This is important for invariant tests
/// where there might be multiple errors traces. Returns `None` if `traces` is
/// empty.
pub fn get_stack_trace<NestedTraceDecoderT: NestedTraceDecoder>(
    contract_decoder: &NestedTraceDecoderT,
    traces: &[(TraceKind, SparsedTraceArena)],
) -> Result<Option<Vec<StackTraceEntry>>, StackTraceError> {
    let mut address_to_creation_code = HashMap::new();
    let mut address_to_runtime_code = HashMap::new();

    for (_, trace) in traces {
        for node in trace.nodes() {
            let address = node.trace.address;
            if node.trace.kind.is_any_create() {
                address_to_creation_code.insert(address, &node.trace.data);
                address_to_runtime_code.insert(address, &node.trace.output);
            }
        }
    }

    if let Some((_, last_trace)) = traces.last() {
        let trace = convert_call_trace_arena_to_nested_trace(
            &address_to_creation_code,
            &address_to_runtime_code,
            last_trace,
        )?;
        let trace = contract_decoder.try_to_decode_nested_trace(trace)?;
        let stack_trace = solidity_tracer::get_stack_trace(trace)?;
        let stack_trace = stack_trace
            .into_iter()
            .filter(|stack_trace| {
                !stack_trace.is_unrecognized_contract_call_error(&CHEATCODE_ADDRESS)
            })
            .collect();
        Ok(Some(stack_trace))
    } else {
        Ok(None)
    }
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

    convert_node_to_nested_trace(address_to_creation_code, address_to_runtime_code, arena, 0)
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
            Bytes::from(trace.address.to_vec())
        } else {
            address_to_runtime_code
                .get(&trace.address)
                // Code might not exist if it's a mocked contract
                // Mimicking behavior here: https://github.com/NomicFoundation/edr/blob/4e7491d8631da27b4bd1ba2bde4914bb704e2c52/crates/foundry/cheatcodes/src/evm/mock.rs#L75
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
}

fn convert_instruction_result_to_exit_code(
    result: revm::interpreter::InstructionResult,
) -> ExitCode {
    let success_or_halt: revm::interpreter::SuccessOrHalt = result.into();
    if success_or_halt.is_success() {
        ExitCode::Success
    } else if success_or_halt.is_revert() {
        ExitCode::Revert
    } else {
        let halt = success_or_halt.to_halt().expect("must be a halt");
        ExitCode::Halt(halt)
    }
}

fn is_calllike_op(step: &CallTraceStep) -> bool {
    use revm::interpreter::opcode;

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

/// The possible outcomes from computing stack traces.
#[derive(Clone, Debug)]
pub enum StackTraceResult {
    /// The stack trace result
    Success(Vec<StackTraceEntry>),
    /// We couldn't generate stack traces, because an unexpected error occurred.
    Error(StackTraceError),
    HeuristicFailed,
    /// We couldn't generate stack traces, because the test execution is unsafe
    /// to replay due to indeterminism. This can be caused by either
    /// specifying a fork url without a fork block number in the test runner
    /// config or using impure cheatcodes.
    UnsafeToReplay {
        /// Indeterminism due to specifying a fork url without a fork block
        /// number in the test runner config
        global_fork_latest: bool,
        /// The list of executed impure cheatcode signatures. We collect
        /// function signatures instead of function names as whether a cheatcode
        /// is impure can depend on the arguments it takes (e.g. `createFork`
        /// without a second argument means implicitly fork from “latest”).
        /// Example signature: `function createSelectFork(string calldata
        /// urlOrAlias) external returns (uint256 forkId);`.
        impure_cheatcodes: Vec<Cow<'static, str>>,
    },
}

impl From<Result<Vec<StackTraceEntry>, StackTraceError>> for StackTraceResult {
    fn from(value: Result<Vec<StackTraceEntry>, StackTraceError>) -> Self {
        match value {
            Ok(stack_trace) => {
                if stack_trace.is_empty() {
                    Self::HeuristicFailed
                } else {
                    Self::Success(stack_trace)
                }
            }
            Err(error) => Self::Error(error),
        }
    }
}

impl From<IndeterminismReasons> for StackTraceResult {
    fn from(value: IndeterminismReasons) -> Self {
        Self::UnsafeToReplay {
            global_fork_latest: value.global_fork_latest,
            impure_cheatcodes: value.impure_cheatcodes,
        }
    }
}
