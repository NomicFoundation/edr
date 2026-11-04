//! Types and utilities for tracing EVM execution with Solidity-specific
//! decoding.

use std::{hash::BuildHasher, sync::Arc};

use edr_chain_spec_evm::{ContextTrait, Inspector};
use edr_primitives::{bytecode::opcode::OpCode, Address, Bytes, HashMap, HashSet, U256};
use parking_lot::RwLock;
use revm_inspector::JournalExt;
use revm_inspectors::tracing::{CallTraceArena, TracingInspector};
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
    pub fn new(inspector: TracingInspector, decoder: Arc<RwLock<ContractDecoder>>) -> Self {
        debug_assert!(
            !inspector.config().record_steps,
            "the wrapped TracingInspector would record every step a second time"
        );

        Self {
            decoder,
            inspector,
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
}
