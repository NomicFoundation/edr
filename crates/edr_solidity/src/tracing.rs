//! Types and utilities for tracing EVM execution with Solidity-specific
//! decoding.

use std::sync::Arc;

use edr_chain_spec_evm::{ContextTrait, Inspector};
use edr_primitives::{Address, Bytes, HashMap, HashSet, U256};
use parking_lot::RwLock;
use revm_inspector::JournalExt;
use revm_inspectors::tracing::{CallTraceArena, TracingInspector};
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
    /// start of each step is captured and returned alongside the collected
    /// traces (see [`CallTraces`]), as a cheap alternative to
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
    ) -> Result<CallTraces, serde_json::Error> {
        let mut arena = self.inspector.into_traces();

        let mut decoder = self.decoder.write();
        decoder.populate_call_trace_arena(
            &mut arena,
            address_to_executed_code,
            precompile_addresses,
        )?;

        Ok(CallTraces {
            arena,
            stack_tops: self.stack_tops,
        })
    }

    /// Takes the [`TracingInspector`]'s traces and ABI decodes them, replacing
    /// the current traces with an empty arena.
    pub fn take(
        &mut self,
        address_to_executed_code: &HashMap<Address, Bytes>,
        precompile_addresses: &HashSet<Address>,
    ) -> Result<CallTraces, serde_json::Error> {
        let mut arena = std::mem::take(self.inspector.traces_mut());
        let stack_tops = std::mem::take(&mut self.stack_tops);

        // Reset the inspector
        self.inspector.fuse();

        let mut decoder = self.decoder.write();
        decoder.populate_call_trace_arena(
            &mut arena,
            address_to_executed_code,
            precompile_addresses,
        )?;

        Ok(CallTraces { arena, stack_tops })
    }
}

/// A call trace arena together with the top-of-stack value captured at the
/// start of each step, in execution order across all call frames.
///
/// The values are kept separate from the arena so that per-step stack
/// snapshots only need to be materialized when the traces are marshalled to a
/// consumer; writing them into the arena's steps would cost one heap
/// allocation per step on every transaction, whether or not the traces are
/// ever read.
///
/// `stack_tops` is empty when top-of-stack recording was disabled (e.g.
/// verbose tracing, which records full stacks in the arena's steps instead).
#[derive(Clone, Debug, Default)]
pub struct CallTraces {
    /// The call trace arena, including ABI-decoded information.
    pub arena: CallTraceArena,
    /// The top-of-stack value at the start of each step, in execution order;
    /// `None` for steps executed with an empty stack.
    pub stack_tops: Vec<Option<U256>>,
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
