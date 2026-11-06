//! Types and utilities for tracing EVM execution with Solidity-specific
//! decoding.

use std::{hash::BuildHasher, sync::Arc};

use edr_chain_spec_evm::{ContextTrait, Inspector};
use edr_primitives::{bytecode::opcode::OpCode, Address, Bytes, HashMap, HashSet, U256};
use parking_lot::RwLock;
use revm_inspector::JournalExt;
use revm_inspectors::tracing::{
    CallTraceArena, StackSnapshotType, TracingInspector, TracingInspectorConfig,
};
use revm_interpreter::{interpreter_types::Jumps, CallOutcome};

use crate::{
    contract_decoder::ContractDecoder,
    recorded_trace_arena::{RecordedStep, RecordedTraceArena, StepRecorder, StepSnapshot},
};

/// Marks the calls to `precompile_addresses` in `arena`, so that
/// [`CallTraceNode::is_precompile`] identifies them.
///
/// The [`TracingInspector`] only identifies precompile calls when it excludes
/// them from the call trace. Like [`TracingInspector::is_precompile_call`],
/// this treats neither the root call nor a call that transfers value as a
/// precompile call.
///
/// [`CallTraceNode::is_precompile`]: revm_inspectors::tracing::types::CallTraceNode::is_precompile
pub fn mark_precompile_calls<HasherT: BuildHasher>(
    arena: &mut CallTraceArena,
    precompile_addresses: &HashSet<Address, HasherT>,
) {
    for node in arena.nodes_mut() {
        let is_precompile_call = node.parent.is_some()
            && node.trace.value.is_zero()
            && precompile_addresses.contains(&node.trace.address);

        node.trace.maybe_precompile = Some(is_precompile_call);
    }
}

/// Arguments of [`SolidityTracingInspector::new`].
#[derive(Clone)]
pub struct SolidityTracingInspectorArgs {
    /// Decodes the contracts of the traced calls.
    pub contract_decoder: Arc<RwLock<ContractDecoder>>,
    /// Whether each step records the memory before it.
    pub record_memory: bool,
    /// How much of the stack each step records.
    pub record_stack: StackSnapshotType,
}

/// Returns the config of the [`TracingInspector`] wrapped by
/// [`SolidityTracingInspector`]. It sets no step limit or opcode filter, since
/// every step should be recorded.
fn tracing_inspector_config(args: &SolidityTracingInspectorArgs) -> TracingInspectorConfig {
    TracingInspectorConfig::none()
        .set_steps(false)
        // Calldata and init code: without them, logs and stack traces
        // cannot identify the called contract and function.
        .set_record_inputs(true)
        .set_memory_snapshots(args.record_memory)
        .set_stack_snapshots(args.record_stack)
        // Call traces include precompile calls, like Solidity test call traces.
        .set_exclude_precompile_calls(false)
        .set_record_logs(true)
}

/// A tracing inspector that uses a [`ContractDecoder`] to decode
/// Solidity-specific information.
///
/// Records the opcode steps of each call itself, in a compact form. The
/// wrapped [`TracingInspector`] is configured without steps, because otherwise
/// every step would be recorded twice.
pub struct SolidityTracingInspector {
    decoder: Arc<RwLock<ContractDecoder>>,
    inspector: TracingInspector,
    /// Steps of each node in the inspector's arena.
    step_recorder: StepRecorder,
}

impl SolidityTracingInspector {
    /// Constructs a new [`SolidityTracingInspector`] instance.
    pub fn new(args: SolidityTracingInspectorArgs) -> Self {
        Self {
            inspector: TracingInspector::new(tracing_inspector_config(&args)),
            decoder: args.contract_decoder,
            step_recorder: StepRecorder::default(),
        }
    }

    /// Collects the [`TracingInspector`]'s traces with the recorded steps and
    /// ABI decodes them.
    pub fn collect(
        self,
        address_to_executed_code: &HashMap<Address, Bytes>,
        precompile_addresses: &HashSet<Address>,
    ) -> Result<RecordedTraceArena, serde_json::Error> {
        let mut arena = self.inspector.into_traces();
        let step_recorder = self.step_recorder;

        let mut decoder = self.decoder.write();
        decoder.populate_call_trace_arena(
            &mut arena,
            address_to_executed_code,
            precompile_addresses,
        )?;

        Ok(step_recorder.finish(arena))
    }

    /// Takes the [`TracingInspector`]'s traces with the recorded steps and ABI
    /// decodes them, replacing them with an empty arena.
    pub fn take(
        &mut self,
        address_to_executed_code: &HashMap<Address, Bytes>,
        precompile_addresses: &HashSet<Address>,
    ) -> Result<RecordedTraceArena, serde_json::Error> {
        let mut arena = std::mem::take(self.inspector.traces_mut());
        let step_recorder = std::mem::take(&mut self.step_recorder);

        // Reset the inspector
        self.inspector.fuse();

        let mut decoder = self.decoder.write();
        decoder.populate_call_trace_arena(
            &mut arena,
            address_to_executed_code,
            precompile_addresses,
        )?;

        Ok(step_recorder.finish(arena))
    }

    /// Records the step about to execute in the call that the
    /// [`TracingInspector`] is tracing, with the stack and memory snapshots
    /// its configuration asks for.
    fn record_step(
        &mut self,
        interp: &revm_interpreter::Interpreter<revm_interpreter::interpreter::EthInterpreter>,
    ) {
        let config = self.inspector.config();
        let stack_data = interp.stack.data();
        let record_stack = config.record_stack_snapshots;
        let node_idx = self
            .inspector
            .active_trace()
            .expect("steps execute inside a call")
            .idx;

        let stack: Option<Box<[U256]>> = if record_stack.is_all() || record_stack.is_full() {
            Some(stack_data.as_slice().into())
        } else if record_stack.is_top() {
            Some(
                stack_data
                    .last()
                    .map(std::slice::from_ref)
                    .unwrap_or_default()
                    .into(),
            )
        } else {
            None
        };

        // Reuses the previous step's memory if that step did not modify it.
        // Taken from `TracingInspector::start_step` (revm-inspectors 0.44.2),
        // so revalidate it when bumping revm-inspectors.
        let memory = config.record_memory_snapshots.then(|| {
            let prev_step = self.step_recorder.last_step(node_idx);
            match prev_step {
                Some(StepSnapshot {
                    op,
                    memory: Some(prev_memory),
                    ..
                }) if !op.modifies_memory() => prev_memory.clone(),
                _ => Bytes::copy_from_slice(&interp.memory.context_memory()),
            }
        });

        let step = RecordedStep {
            pc: u32::try_from(interp.bytecode.pc()).expect("PC larger than u32::MAX"),
            op: OpCode::new_or_unknown(interp.bytecode.opcode()),
        };

        self.step_recorder.record(node_idx, step, stack, memory);
    }
}

impl<ContextT: ContextTrait<Journal: JournalExt>> Inspector<ContextT> for SolidityTracingInspector {
    fn initialize_interp(
        &mut self,
        interp: &mut revm_interpreter::Interpreter<revm_interpreter::interpreter::EthInterpreter>,
        context: &mut ContextT,
    ) {
        self.inspector.initialize_interp(interp, context);
    }

    fn step(
        &mut self,
        interp: &mut revm_interpreter::Interpreter<revm_interpreter::interpreter::EthInterpreter>,
        context: &mut ContextT,
    ) {
        self.inspector.step(interp, context);
        self.record_step(interp);
    }

    fn step_end(
        &mut self,
        interp: &mut revm_interpreter::Interpreter<revm_interpreter::interpreter::EthInterpreter>,
        context: &mut ContextT,
    ) {
        self.inspector.step_end(interp, context);
    }

    fn log(&mut self, context: &mut ContextT, log: alloy_primitives::Log) {
        self.inspector.log(context, log);
    }

    fn log_full(
        &mut self,
        revm_interpreter: &mut revm_interpreter::Interpreter<
            revm_interpreter::interpreter::EthInterpreter,
        >,
        context: &mut ContextT,
        log: alloy_primitives::Log,
    ) {
        self.inspector.log_full(revm_interpreter, context, log);
    }

    fn call(
        &mut self,
        context: &mut ContextT,
        inputs: &mut revm_interpreter::CallInputs,
    ) -> Option<CallOutcome> {
        self.inspector.call(context, inputs)
    }

    fn call_end(
        &mut self,
        context: &mut ContextT,
        inputs: &revm_interpreter::CallInputs,
        outcome: &mut CallOutcome,
    ) {
        self.inspector.call_end(context, inputs, outcome);
    }

    fn create(
        &mut self,
        context: &mut ContextT,
        inputs: &mut revm_interpreter::CreateInputs,
    ) -> Option<revm_interpreter::CreateOutcome> {
        self.inspector.create(context, inputs)
    }

    fn create_end(
        &mut self,
        context: &mut ContextT,
        inputs: &revm_interpreter::CreateInputs,
        outcome: &mut revm_interpreter::CreateOutcome,
    ) {
        self.inspector.create_end(context, inputs, outcome);
    }

    fn selfdestruct(&mut self, contract: Address, target: Address, value: U256) {
        Inspector::<ContextT>::selfdestruct(&mut self.inspector, contract, target, value);
    }
}

#[cfg(test)]
mod tests {
    use edr_primitives::address;
    use revm_inspectors::tracing::types::{CallTrace, CallTraceNode};

    use super::*;

    const PRECOMPILE: Address = address!("0x0000000000000000000000000000000000000004");
    const CONTRACT: Address = address!("0x5FbDB2315678afecb367f032d93F642f64180aa3");
    const ROOT_IDX: usize = 0;

    /// Builds an arena whose root calls each of `child_calls`, given as
    /// `(address, value)`.
    fn arena_with_root_calls(
        root_address: Address,
        child_calls: &[(Address, U256)],
    ) -> CallTraceArena {
        let mut arena = CallTraceArena::default();
        arena.nodes_mut()[ROOT_IDX].trace.address = root_address;

        for &(address, value) in child_calls {
            let idx = arena.nodes().len();
            arena.nodes_mut()[ROOT_IDX].children.push(idx);
            arena.nodes_mut().push(CallTraceNode {
                parent: Some(ROOT_IDX),
                idx,
                trace: CallTrace {
                    address,
                    value,
                    ..CallTrace::default()
                },
                ..CallTraceNode::default()
            });
        }

        arena
    }

    fn precompile_addresses() -> HashSet<Address> {
        HashSet::from_iter([PRECOMPILE])
    }

    fn precompile_flags(arena: &CallTraceArena) -> Vec<bool> {
        arena
            .nodes()
            .iter()
            .map(CallTraceNode::is_precompile)
            .collect()
    }

    #[test]
    fn mark_precompile_calls_marks_value_free_child_calls_to_precompiles() {
        let mut arena = arena_with_root_calls(
            CONTRACT,
            &[
                (PRECOMPILE, U256::ZERO),
                (PRECOMPILE, U256::from(1)),
                (CONTRACT, U256::ZERO),
            ],
        );

        mark_precompile_calls(&mut arena, &precompile_addresses());

        assert_eq!(precompile_flags(&arena), [false, true, false, false]);
    }

    #[test]
    fn mark_precompile_calls_skips_root_call_to_precompile() {
        let mut arena = arena_with_root_calls(PRECOMPILE, &[]);

        mark_precompile_calls(&mut arena, &precompile_addresses());

        assert_eq!(precompile_flags(&arena), [false]);
    }

    /// `SolidityTracingInspector` records the same steps that
    /// `TracingInspector` records in its own `CallTraceStep`s, for every
    /// call frame and every stack and memory snapshot setting.
    mod step_recording {
        use edr_primitives::bytes;
        use edr_test_utils::bytecode::{opcode, BytecodeBuilder};
        use revm::{
            bytecode::Bytecode,
            context::TxEnv,
            context_interface::TransactTo,
            database::CacheDB,
            database_interface::EmptyDB,
            interpreter::{CallInputs, Gas, InstructionResult, InterpreterResult},
            primitives::hardfork::SpecId,
            state::AccountInfo,
            Context, InspectEvm, MainBuilder, MainContext,
        };
        use revm_inspectors::tracing::types::RecordedMemory;

        use super::*;

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
        /// An arbitrary value that frames write to memory.
        const MEMORY_WORD: u8 = 0x2a;

        /// Appends a call to `to` with `call_op`, discarding the success flag.
        /// `value` is only used by `CALL`.
        fn call<'code>(
            code: &'code mut BytecodeBuilder,
            call_op: u8,
            to: Address,
            value: &[u8],
        ) -> &'code mut BytecodeBuilder {
            // retSize, retOffset, argsSize, argsOffset
            code.push1(0x20).push1(0x40).push1(0x20).push1(0);
            if call_op == opcode::CALL {
                code.push(value);
            }
            code.push(to.as_slice())
                .opcode(opcode::GAS)
                .opcode(call_op)
                .opcode(opcode::POP)
        }

        fn root_code() -> Bytes {
            // Init code of a contract with empty runtime code: PUSH1 0 PUSH1 0 RETURN
            let init_code = [opcode::PUSH1, 0, opcode::PUSH1, 0, opcode::RETURN];
            let init_code_len = u8::try_from(init_code.len()).expect("init code is short");

            let mut code = BytecodeBuilder::default();
            code.push1(MEMORY_WORD).push1(0).opcode(opcode::MSTORE);
            call(&mut code, opcode::CALL, CALLEE, &[0]);
            call(&mut code, opcode::STATICCALL, PRECOMPILE, &[]);
            call(&mut code, opcode::CALL, EMPTY, &[0]);
            call(&mut code, opcode::CALL, MOCKED, &[0]);
            // Fails before the callee starts because the value exceeds ROOT's balance.
            call(&mut code, opcode::CALL, CALLEE, &[0xff; 32]);
            call(&mut code, opcode::DELEGATECALL, CALLEE, &[]);
            call(&mut code, opcode::CALL, REVERTER, &[0]);
            // CREATE(value, offset, size) with the init code right-aligned in the
            // first memory word.
            code.push(&init_code)
                .push1(0)
                .opcode(opcode::MSTORE)
                .push1(init_code_len)
                .push1(32 - init_code_len)
                .push1(0)
                .opcode(opcode::CREATE)
                .opcode(opcode::POP);
            // Last, since the halt consumes all the gas forwarded to it.
            call(&mut code, opcode::CALL, HALTER, &[0]);
            code.opcode(opcode::STOP);
            code.runtime()
        }

        fn callee_code() -> Bytes {
            let mut code = BytecodeBuilder::default();
            code.push1(MEMORY_WORD)
                .push1(0)
                .opcode(opcode::MSTORE)
                .push1(0)
                .opcode(opcode::MLOAD)
                .push1(1)
                .opcode(opcode::SSTORE)
                .push1(0x20)
                .push1(0)
                .opcode(opcode::RETURN);
            code.runtime()
        }

        fn reverter_code() -> Bytes {
            let mut code = BytecodeBuilder::default();
            code.push1(0).push1(0).opcode(opcode::REVERT);
            code.runtime()
        }

        /// Answers calls to [`MOCKED`] without executing them, like EDR's call
        /// override.
        struct MockCalls;

        impl<ContextT> Inspector<ContextT> for MockCalls {
            fn call(
                &mut self,
                _context: &mut ContextT,
                inputs: &mut CallInputs,
            ) -> Option<CallOutcome> {
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

        /// Executes a call to `to` with `TracingInspector` recording steps and
        /// with `SolidityTracingInspector` recording them instead, side
        /// by side.
        fn trace_both(
            to: Address,
            args: SolidityTracingInspectorArgs,
        ) -> (CallTraceArena, RecordedTraceArena) {
            let reference = TracingInspector::new(tracing_inspector_config(&args).set_steps(true));
            let recorder = SolidityTracingInspector::new(args);

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
            // `nodes()` stops at the first node without a steps entry.
            assert_eq!(reference.nodes().len(), recorded.nodes().len());

            for (node_idx, node) in reference.nodes().iter().enumerate() {
                let expected = &node.trace.steps;
                let steps = recorded
                    .node(node_idx)
                    .expect("steps are recorded for every node")
                    .steps();
                assert_eq!(steps.len(), expected.len(), "step count of node {node_idx}");

                for (step_idx, (step, expected)) in steps.iter().zip(expected).enumerate() {
                    let context = format!("node {node_idx}, step {step_idx}");
                    assert_eq!(step.pc as usize, expected.pc, "pc at {context}");
                    assert_eq!(step.op, expected.op, "op at {context}");
                    assert_eq!(step.stack, expected.stack.as_deref(), "stack at {context}");
                    assert_eq!(
                        step.memory,
                        expected.memory.as_ref().map(RecordedMemory::as_bytes),
                        "memory at {context}"
                    );
                }
            }
        }

        fn args(
            record_memory: bool,
            record_stack: StackSnapshotType,
        ) -> SolidityTracingInspectorArgs {
            SolidityTracingInspectorArgs {
                contract_decoder: Arc::new(RwLock::new(ContractDecoder::default())),
                record_memory,
                record_stack,
            }
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
                        let (reference, recorded) =
                            trace_both(to, args(record_memory, stack_snapshot));
                        assert_same_steps(&reference, &recorded);
                    }
                }
            }
        }

        #[test]
        fn tracing_inspector_config_sets_no_step_limit_or_opcode_filter() {
            let config = tracing_inspector_config(&args(true, StackSnapshotType::Full));

            assert!(config.step_limit.is_none());
            assert!(config.record_opcodes_filter.is_none());
        }

        #[test]
        fn covers_frames_without_steps() {
            let (reference, recorded) = trace_both(ROOT, args(false, StackSnapshotType::None));

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
    }
}
