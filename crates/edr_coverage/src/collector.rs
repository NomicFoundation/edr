use edr_eth::{Bytes, HashSet};
use edr_evm::{inspector::Inspector, interpreter::InterpreterTypes};

use crate::COVERAGE_ADDRESS;

#[derive(Default)]
pub struct CoverageHitCollector {
    hits: HashSet<Bytes>,
}

impl CoverageHitCollector {
    /// Returns the collected coverage hits.
    pub fn into_hits(self) -> HashSet<Bytes> {
        self.hits
    }

    fn record_hit(&mut self, hit: Bytes) {
        self.hits.insert(hit);
    }
}

impl<ContextT, InterpreterT: InterpreterTypes> Inspector<ContextT, InterpreterT>
    for CoverageHitCollector
{
    fn call(
        &mut self,
        _context: &mut ContextT,
        inputs: &mut edr_evm::interpreter::CallInputs,
    ) -> Option<edr_evm::interpreter::CallOutcome> {
        if inputs.bytecode_address == COVERAGE_ADDRESS {
            self.record_hit(inputs.input.clone());
        }

        None
    }
}
