//! Checks that [`SolidityTracingInspector`] records the same steps that
//! [`TracingInspector`] records in its own `CallTraceStep`s, for every call
//! frame and every stack and memory snapshot setting.

use std::sync::Arc;

use edr_primitives::{address, bytes, Address, Bytes, HashMap, HashSet, U256};
use edr_solidity::{
    contract_decoder::ContractDecoder, recorded_steps::RecordedTraceArena,
    tracing::SolidityTracingInspector,
};
use parking_lot::RwLock;
use revm::{
    bytecode::{opcode, Bytecode},
    context::TxEnv,
    context_interface::TransactTo,
    database::CacheDB,
    database_interface::EmptyDB,
    interpreter::{CallInputs, CallOutcome, Gas, InstructionResult, InterpreterResult},
    primitives::hardfork::SpecId,
    state::AccountInfo,
    Context, InspectEvm, Inspector, MainBuilder, MainContext,
};
use revm_inspectors::tracing::{
    types::RecordedMemory, CallTraceArena, StackSnapshotType, TracingInspector,
    TracingInspectorConfig,
};

const CALLER: Address = address!("0x1000000000000000000000000000000000000001");
/// Exercises every kind of call frame.
const ROOT: Address = address!("0xa000000000000000000000000000000000000001");
/// Writes and reads memory and storage, then returns.
const CALLEE: Address = address!("0xb000000000000000000000000000000000000001");
/// Has no code.
const EMPTY: Address = address!("0xc000000000000000000000000000000000000001");
/// Answered by [`MockCalls`] without executing.
const MOCKED: Address = address!("0xd000000000000000000000000000000000000001");
const REVERTER: Address = address!("0xe000000000000000000000000000000000000001");
const HALTER: Address = address!("0xf000000000000000000000000000000000000001");
const IDENTITY_PRECOMPILE: Address = address!("0x0000000000000000000000000000000000000004");

/// Builds bytecode from opcodes and `PUSH` immediates.
#[derive(Default)]
struct Asm(Vec<u8>);

impl Asm {
    fn op(mut self, op: u8) -> Self {
        self.0.push(op);
        self
    }

    fn push(mut self, bytes: &[u8]) -> Self {
        let len = u8::try_from(bytes.len()).expect("push fits in 32 bytes");
        assert!((1..=32).contains(&len));
        self.0.push(opcode::PUSH0 + len);
        self.0.extend_from_slice(bytes);
        self
    }

    fn push_u8(self, value: u8) -> Self {
        self.push(&[value])
    }

    /// Calls `to` with `call_op`, discarding the success flag. `value` is
    /// ignored for `STATICCALL` and `DELEGATECALL`.
    fn call(self, call_op: u8, to: Address, value: &[u8]) -> Self {
        // retSize, retOffset, argsSize, argsOffset
        let asm = self.push_u8(0x20).push_u8(0x40).push_u8(0x20).push_u8(0);
        let asm = if matches!(call_op, opcode::CALL | opcode::CALLCODE) {
            asm.push(value)
        } else {
            asm
        };
        asm.push(to.as_slice())
            .op(opcode::GAS)
            .op(call_op)
            .op(opcode::POP)
    }

    fn build(self) -> Bytes {
        self.0.into()
    }
}

fn root_code() -> Bytes {
    // Init code of a contract with empty runtime code: PUSH1 0 PUSH1 0 RETURN
    let init_code = [opcode::PUSH1, 0, opcode::PUSH1, 0, opcode::RETURN];

    Asm::default()
        .push_u8(0x2a)
        .push_u8(0)
        .op(opcode::MSTORE)
        .call(opcode::CALL, CALLEE, &[0])
        .call(opcode::STATICCALL, IDENTITY_PRECOMPILE, &[])
        .call(opcode::CALL, EMPTY, &[0])
        .call(opcode::CALL, MOCKED, &[0])
        // Fails before the callee starts: the value exceeds ROOT's balance.
        .call(opcode::CALL, CALLEE, &[0xff; 32])
        .call(opcode::DELEGATECALL, CALLEE, &[])
        .call(opcode::CALL, REVERTER, &[0])
        .call(opcode::CALL, HALTER, &[0])
        // CREATE(value, offset, size) with the init code right-aligned in the
        // first memory word.
        .push(&init_code)
        .push_u8(0)
        .op(opcode::MSTORE)
        .push_u8(u8::try_from(init_code.len()).expect("init code is short"))
        .push_u8(32 - u8::try_from(init_code.len()).expect("init code is short"))
        .push_u8(0)
        .op(opcode::CREATE)
        .op(opcode::POP)
        .op(opcode::STOP)
        .build()
}

fn callee_code() -> Bytes {
    Asm::default()
        .push_u8(0x2a)
        .push_u8(0)
        .op(opcode::MSTORE)
        .push_u8(0)
        .op(opcode::MLOAD)
        .push_u8(1)
        .op(opcode::SSTORE)
        .push_u8(0x20)
        .push_u8(0)
        .op(opcode::RETURN)
        .build()
}

fn reverter_code() -> Bytes {
    Asm::default()
        .push_u8(0)
        .push_u8(0)
        .op(opcode::REVERT)
        .build()
}

/// Answers calls to [`MOCKED`] without executing them, like EDR's call
/// override.
struct MockCalls;

impl<ContextT> Inspector<ContextT> for MockCalls {
    fn call(&mut self, _context: &mut ContextT, inputs: &mut CallInputs) -> Option<CallOutcome> {
        (inputs.target_address == MOCKED).then(|| {
            CallOutcome::new(
                InterpreterResult {
                    result: InstructionResult::Return,
                    output: bytes!("0x01"),
                    gas: Gas::new(inputs.gas_limit),
                },
                inputs.return_memory_offset.clone(),
            )
        })
    }
}

fn database() -> CacheDB<EmptyDB> {
    let mut db = CacheDB::new(EmptyDB::default());
    db.insert_account_info(
        CALLER,
        AccountInfo {
            balance: U256::from(10u64).pow(U256::from(18)),
            ..AccountInfo::default()
        },
    );
    for (address, code) in [
        (ROOT, root_code()),
        (CALLEE, callee_code()),
        (REVERTER, reverter_code()),
        (HALTER, Bytes::from_static(&[opcode::INVALID])),
    ] {
        db.insert_account_info(
            address,
            AccountInfo {
                balance: U256::from(1),
                code: Some(Bytecode::new_raw(code)),
                ..AccountInfo::default()
            },
        );
    }
    db
}

/// Executes a call to `to` with `TracingInspector` recording steps and with
/// `SolidityTracingInspector` recording them instead, side by side.
fn trace_both(to: Address, config: TracingInspectorConfig) -> (CallTraceArena, RecordedTraceArena) {
    let reference = TracingInspector::new(config.set_steps(true));
    let recorder = SolidityTracingInspector::new(
        TracingInspector::new(config.set_steps(false)),
        Arc::new(RwLock::new(ContractDecoder::default())),
    );

    let mut evm = Context::mainnet()
        .with_db(database())
        .modify_cfg_chained(|cfg| cfg.spec = SpecId::PRAGUE)
        .build_mainnet_with_inspector((MockCalls, (reference, recorder)));
    evm.inspect_tx(TxEnv {
        caller: CALLER,
        gas_limit: 1_000_000,
        kind: TransactTo::Call(to),
        ..TxEnv::default()
    })
    .expect("transaction is valid");

    let (_, (reference, recorder)) = evm.inspector;
    let recorded = recorder
        .collect(&HashMap::default(), &HashSet::default())
        .expect("no ABI to decode");
    (reference.into_traces(), recorded)
}

fn assert_same_steps(reference: &CallTraceArena, recorded: &RecordedTraceArena) {
    assert_eq!(reference.nodes().len(), recorded.nodes().len());

    for (node_idx, node) in reference.nodes().iter().enumerate() {
        let expected = &node.trace.steps;
        let steps = recorded.node_steps(node_idx);
        assert_eq!(steps.len(), expected.len(), "step count of node {node_idx}");

        for (step_idx, (step, expected)) in steps.iter().zip(expected).enumerate() {
            let context = format!("node {node_idx}, step {step_idx}");
            assert_eq!(step.pc as usize, expected.pc, "pc at {context}");
            assert_eq!(step.op, expected.op, "op at {context}");

            let node_steps = recorded.node(node_idx);
            assert_eq!(
                node_steps.and_then(|node| node.stack(step_idx)),
                expected.stack.as_deref(),
                "stack at {context}"
            );
            assert_eq!(
                node_steps.and_then(|node| node.memory(step_idx)),
                expected.memory.as_ref().map(RecordedMemory::as_bytes),
                "memory at {context}"
            );
        }
    }
}

/// The provider's settings that affect which call frames are traced.
fn edr_config() -> TracingInspectorConfig {
    TracingInspectorConfig::none()
        .set_record_inputs(true)
        .set_exclude_precompile_calls(false)
        .set_record_logs(true)
}

#[test]
fn records_the_same_steps_as_tracing_inspector() {
    let stack_snapshots = [
        StackSnapshotType::None,
        StackSnapshotType::Top,
        StackSnapshotType::Full,
        StackSnapshotType::All,
    ];

    for to in [ROOT, REVERTER, HALTER, EMPTY] {
        for stack_snapshot in stack_snapshots {
            for record_memory in [false, true] {
                let config = edr_config()
                    .set_stack_snapshots(stack_snapshot)
                    .set_memory_snapshots(record_memory);

                let (reference, recorded) = trace_both(to, config);
                assert_same_steps(&reference, &recorded);
            }
        }
    }
}

#[test]
fn covers_frames_without_steps() {
    let (reference, recorded) = trace_both(ROOT, edr_config());

    let frames_without_steps = reference
        .nodes()
        .iter()
        .filter(|node| node.trace.steps.is_empty())
        .count();
    // The precompile, the empty account, the mocked call and the call that
    // fails before starting.
    assert!(
        frames_without_steps >= 4,
        "expected frames without steps, got {frames_without_steps}"
    );
    assert_same_steps(&reference, &recorded);
}
