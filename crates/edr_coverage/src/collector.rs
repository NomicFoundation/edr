use edr_chain_spec_evm::{
    interpreter::{
        CallInputs, CallOutcome, CreateInputs, CreateOutcome, Gas, InstructionResult,
        InterpreterTypes,
    },
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
    /// Flushes the collected coverage hits, replacing the current hits with an
    /// empty set, and returning the previous hits.
    ///
    /// Also resets the previous call output to an empty byte array, which is
    /// important to ensure that subsequent transactions do not retain return
    /// data from a prior transaction.
    pub fn flush(&mut self) -> HashSet<Bytes> {
        let hits = std::mem::take(&mut self.hits);
        self.previous_call_output = Bytes::new();
        hits
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

            // Short-circuit the call to avoid execution of empty bytecode—which results in
            // a `InstructionResult::Stop`—instead replaying the previous call or create's
            // output to preserve the returndata buffer.
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
        inputs: &CallInputs,
        outcome: &mut CallOutcome,
    ) {
        // Skip coverage calls — their output is already identical to what we stored.
        if inputs.bytecode_address != COVERAGE_ADDRESS {
            self.previous_call_output = outcome.result.output.clone();
        }
    }

    fn create_end(
        &mut self,
        _context: &mut ContextT,
        _inputs: &CreateInputs,
        outcome: &mut CreateOutcome,
    ) {
        self.previous_call_output = outcome.result.output.clone();
    }
}
