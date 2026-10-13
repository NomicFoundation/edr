//! Types and utilities for tracing EVM execution with Solidity-specific
//! decoding.

use std::{hash::BuildHasher, sync::Arc};

use edr_chain_spec_evm::{ContextTrait, Inspector};
use edr_primitives::{Address, Bytes, HashMap, HashSet, U256};
use parking_lot::RwLock;
use revm_inspector::JournalExt;
use revm_inspectors::tracing::{CallTraceArena, TracingInspector};
use revm_interpreter::CallOutcome;

use crate::contract_decoder::ContractDecoder;

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
pub struct SolidityTracingInspector {
    decoder: Arc<RwLock<ContractDecoder>>,
    inspector: TracingInspector,
}

impl SolidityTracingInspector {
    /// Constructs a new [`SolidityTracingInspector`] instance.
    pub fn new(inspector: TracingInspector, decoder: Arc<RwLock<ContractDecoder>>) -> Self {
        Self { decoder, inspector }
    }

    /// Collects the [`TracingInspector`]'s traces and ABI decodes them.
    pub fn collect(
        self,
        address_to_executed_code: &HashMap<Address, Bytes>,
        precompile_addresses: &HashSet<Address>,
    ) -> Result<CallTraceArena, serde_json::Error> {
        let mut arena = self.inspector.into_traces();

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

        // Reset the inspector
        self.inspector.fuse();

        let mut decoder = self.decoder.write();
        decoder.populate_call_trace_arena(
            &mut arena,
            address_to_executed_code,
            precompile_addresses,
        )?;

        Ok(arena)
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
