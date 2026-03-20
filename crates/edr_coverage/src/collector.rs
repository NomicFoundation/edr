use edr_chain_spec_evm::{
    interpreter::{CallInputs, CallOutcome, Gas, InstructionResult, InterpreterTypes},
    ContextTrait, Inspector, InterpreterResult,
};
use edr_primitives::{Bytes, HashSet};

use crate::COVERAGE_ADDRESS;

#[derive(Clone, Debug, Default)]
pub struct CoverageHitCollector {
    hits: HashSet<Bytes>,
    /// Stores the output of the previous call so that when an instrumentation
    /// call is identified, the collector can mimic the previous call's output,
    /// preventing the instrumentation from interfering with the rest of the
    /// execution.
    previous_call_output: Bytes,
}

impl CoverageHitCollector {
    /// Replaces the current hits with an empty set, returning the previous
    /// hits.
    pub fn take(&mut self) -> HashSet<Bytes> {
        std::mem::take(&mut self.hits)
    }

    /// Returns the collected coverage hits.
    pub fn into_hits(self) -> HashSet<Bytes> {
        self.hits
    }

    fn record_hit(&mut self, hit: Bytes) {
        self.hits.insert(hit);
    }
}

impl<ContextT: ContextTrait, InterpreterT: InterpreterTypes> Inspector<ContextT, InterpreterT>
    for CoverageHitCollector
{
    fn call(&mut self, context: &mut ContextT, inputs: &mut CallInputs) -> Option<CallOutcome> {
        if inputs.bytecode_address == COVERAGE_ADDRESS {
            self.record_hit(inputs.input.bytes(context));

            // Short-circuit the call to avoid on-chain execution, replaying
            // the previous call's output to preserve the returndata buffer.
            Some(CallOutcome {
                result: InterpreterResult {
                    result: InstructionResult::Return,
                    output: self.previous_call_output.clone(),
                    gas: Gas::new(inputs.gas_limit),
                },
                memory_offset: inputs.return_memory_offset.clone(),
                was_precompile_called: false,
                precompile_call_logs: vec![],
            })
        } else {
            None
        }
    }

    fn call_end(
        &mut self,
        _context: &mut ContextT,
        _inputs: &CallInputs,
        outcome: &mut CallOutcome,
    ) {
        // Note: This also fires for short-circuited coverage calls (via
        // `inspector.call() -> Some`), writing back the same output we
        // already stored — effectively a no-op. We intentionally don't
        // filter those out to keep the code simple.
        let InterpreterResult { output, .. } = &outcome.result;
        // Safe to store unconditionally — the interpreter always overwrites the
        // returndata buffer from the call outcome's output.
        // See `EthFrame::return_result` in revm/crates/handler/src/frame.rs.
        self.previous_call_output = output.clone();
    }
}
