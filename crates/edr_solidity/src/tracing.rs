//! Types and utilities for tracing EVM execution with Solidity-specific
//! decoding.

use std::sync::Arc;

use edr_chain_spec_evm::{ContextTrait, Inspector};
use edr_primitives::{Address, Bytes, HashMap, HashSet, U256};
use parking_lot::RwLock;
use revm_inspector::JournalExt;
use revm_inspectors::tracing::{
    types::{CallTraceNode, TraceMemberOrder},
    CallTraceArena, TracingInspector,
};
use revm_interpreter::CallOutcome;

use crate::contract_decoder::ContractDecoder;

/// A tracing inspector that uses a [`ContractDecoder`] to decode
/// Solidity-specific information.
pub struct SolidityTracingInspector {
    decoder: Arc<RwLock<ContractDecoder>>,
    inspector: TracingInspector,
    record_top_of_stack: bool,
    stack_tops: Vec<Option<U256>>,
}

impl SolidityTracingInspector {
    /// Constructs a new [`SolidityTracingInspector`] instance.
    ///
    /// When `record_top_of_stack` is enabled, the top-of-stack value at the
    /// start of each step is captured and written into the collected traces'
    /// steps' `stack` field, as a cheap alternative to
    /// [`revm_inspectors::tracing::StackSnapshotType::Full`], which clones the
    /// entire stack on every step.
    pub fn new(
        inspector: TracingInspector,
        decoder: Arc<RwLock<ContractDecoder>>,
        record_top_of_stack: bool,
    ) -> Self {
        debug_assert!(
            !record_top_of_stack
                || (inspector.config().record_steps
                    && inspector.config().record_opcodes_filter.is_none()),
            "captured stack tops are matched to steps by position, so every step must be recorded"
        );

        Self {
            decoder,
            inspector,
            record_top_of_stack,
            stack_tops: Vec::new(),
        }
    }

    /// Collects the [`TracingInspector`]'s traces and ABI decodes them.
    pub fn collect(
        self,
        address_to_executed_code: &HashMap<Address, Bytes>,
        precompile_addresses: &HashSet<Address>,
    ) -> Result<CallTraceArena, serde_json::Error> {
        let mut arena = self.inspector.into_traces();

        write_stack_tops(&mut arena, &self.stack_tops);

        let mut decoder = self.decoder.write();
        decoder.populate_call_trace_arena(
            &mut arena,
            address_to_executed_code,
            precompile_addresses,
        )?;

        Ok(arena)
    }

    /// Takes the [`TracingInspector`]'s traces and ABI decodes them, replacing
    /// the current traces with an empty arena.
    pub fn take(
        &mut self,
        address_to_executed_code: &HashMap<Address, Bytes>,
        precompile_addresses: &HashSet<Address>,
    ) -> Result<CallTraceArena, serde_json::Error> {
        let mut arena = std::mem::take(self.inspector.traces_mut());
        let stack_tops = std::mem::take(&mut self.stack_tops);

        // Reset the inspector
        self.inspector.fuse();

        write_stack_tops(&mut arena, &stack_tops);

        let mut decoder = self.decoder.write();
        decoder.populate_call_trace_arena(
            &mut arena,
            address_to_executed_code,
            precompile_addresses,
        )?;

        Ok(arena)
    }
}

/// Writes captured per-step top-of-stack values into the arena's steps,
/// consuming them in execution order: each node's `ordering`, descending
/// depth-first into child calls.
fn write_stack_tops(arena: &mut CallTraceArena, stack_tops: &[Option<U256>]) {
    if stack_tops.is_empty() {
        return;
    }

    let mut tops = stack_tops.iter();
    write_node_stack_tops(arena.nodes_mut(), 0, &mut tops);
    debug_assert_eq!(
        tops.len(),
        0,
        "captured more stack tops than recorded steps"
    );
}

fn write_node_stack_tops(
    nodes: &mut [CallTraceNode],
    node_idx: usize,
    tops: &mut std::slice::Iter<'_, Option<U256>>,
) {
    let ordering = nodes
        .get(node_idx)
        .expect("node index should be valid")
        .ordering
        .clone();

    for entry in ordering {
        match entry {
            TraceMemberOrder::Step(step_idx) => {
                let Some(top) = tops.next() else {
                    debug_assert!(false, "recorded more steps than captured stack tops");
                    return;
                };

                let step = nodes
                    .get_mut(node_idx)
                    .expect("node index should be valid")
                    .trace
                    .steps
                    .get_mut(step_idx)
                    .expect("step index should be valid");
                step.stack = top.map(|top| Box::from([top]));
            }
            TraceMemberOrder::Call(child_idx) => {
                let child_node_idx = *nodes
                    .get(node_idx)
                    .expect("node index should be valid")
                    .children
                    .get(child_idx)
                    .expect("child index should be valid");
                write_node_stack_tops(nodes, child_node_idx, tops);
            }
            TraceMemberOrder::Log(_) => {}
        }
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
        if self.record_top_of_stack {
            self.stack_tops.push(interp.stack.data().last().copied());
        }
        self.inspector.step(interp, context);
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
    use edr_primitives::bytecode::opcode::OpCode;
    use revm_inspectors::tracing::types::{CallTraceStep, TraceMemberOrder};

    use super::*;

    fn step() -> CallTraceStep {
        CallTraceStep {
            pc: 0,
            op: OpCode::STOP,
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

    /// Root with three steps interleaved with a log and a child call:
    /// execution order is root step 0, root step 1, child steps 0-1, root
    /// step 2.
    fn nested_arena() -> CallTraceArena {
        let mut arena = CallTraceArena::default();

        let root = &mut arena.nodes_mut()[0];
        root.children = vec![1];
        root.trace.steps = vec![step(), step(), step()];
        root.ordering = vec![
            TraceMemberOrder::Step(0),
            TraceMemberOrder::Log(0),
            TraceMemberOrder::Step(1),
            TraceMemberOrder::Call(0),
            TraceMemberOrder::Step(2),
        ];

        let mut child = CallTraceNode {
            parent: Some(0),
            idx: 1,
            ..CallTraceNode::default()
        };
        child.trace.steps = vec![step(), step()];
        child.ordering = vec![TraceMemberOrder::Step(0), TraceMemberOrder::Step(1)];
        arena.nodes_mut().push(child);

        arena
    }

    fn stack_of(arena: &CallTraceArena, node_idx: usize, step_idx: usize) -> Option<&[U256]> {
        arena.nodes()[node_idx].trace.steps[step_idx]
            .stack
            .as_deref()
    }

    #[test]
    fn write_stack_tops_follows_execution_order_across_frames() {
        let mut arena = nested_arena();

        write_stack_tops(
            &mut arena,
            &[
                None,
                Some(U256::from(1)),
                Some(U256::from(10)),
                Some(U256::from(11)),
                Some(U256::from(2)),
            ],
        );

        assert_eq!(stack_of(&arena, 0, 0), None);
        assert_eq!(stack_of(&arena, 0, 1), Some(&[U256::from(1)][..]));
        assert_eq!(stack_of(&arena, 0, 2), Some(&[U256::from(2)][..]));
        assert_eq!(stack_of(&arena, 1, 0), Some(&[U256::from(10)][..]));
        assert_eq!(stack_of(&arena, 1, 1), Some(&[U256::from(11)][..]));
    }

    #[test]
    fn write_stack_tops_without_captured_values_preserves_recorded_stacks() {
        let mut arena = nested_arena();
        let full_stack = [U256::from(1), U256::from(2)];
        arena.nodes_mut()[0].trace.steps[0].stack = Some(Box::from(full_stack));

        write_stack_tops(&mut arena, &[]);

        assert_eq!(stack_of(&arena, 0, 0), Some(&full_stack[..]));
        assert_eq!(stack_of(&arena, 0, 1), None);
        assert_eq!(stack_of(&arena, 0, 2), None);
        assert_eq!(stack_of(&arena, 1, 0), None);
        assert_eq!(stack_of(&arena, 1, 1), None);
    }
}
