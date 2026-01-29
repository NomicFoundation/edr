use edr_chain_spec_evm::{
    interpreter::{CallInputs, CallOutcome, InterpreterTypes},
    ContextTrait, Inspector,
};
use edr_primitives::{Bytes, HashSet};

use crate::COVERAGE_ADDRESS;

#[derive(Clone, Debug, Default)]
pub struct CoverageHitCollector {
    hits: HashSet<Bytes>,
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
        }

        None
    }
}
