use edr_chain_spec_evm::{
    interpreter::{CallInputs, CallOutcome, InputsTr as _, InterpreterTypes, ReturnData as _},
    ContextTrait, Inspector,
};
use edr_primitives::{Bytes, HashSet};

use crate::COVERAGE_ADDRESS;

#[derive(Clone, Debug, Default)]
pub struct CoverageHitCollector {
    hits: HashSet<Bytes>,
    /// Stores the return data of the previous call, which is used by
    /// `returndatacopy` and `returndatasize`.
    previous_call_return_data: Bytes,
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
    fn initialize_interp(
        &mut self,
        interp: &mut edr_chain_spec_evm::interpreter::Interpreter<InterpreterT>,
        _context: &mut ContextT,
    ) {
        self.previous_call_return_data = interp.return_data.buffer().clone();
        println!(
            "initialize_interp ({:?}): {:?}",
            interp.input.bytecode_address(),
            self.previous_call_return_data
        );
    }

    fn call(&mut self, context: &mut ContextT, inputs: &mut CallInputs) -> Option<CallOutcome> {
        println!("call: {}", inputs.bytecode_address);
        if inputs.bytecode_address == COVERAGE_ADDRESS {
            self.record_hit(inputs.input.bytes(context));
        }

        None
    }

    fn call_end(
        &mut self,
        _context: &mut ContextT,
        _inputs: &CallInputs,
        outcome: &mut CallOutcome,
    ) {
        println!("call end: {}", _inputs.bytecode_address);
        outcome.result.output = self.previous_call_return_data.clone();
    }
}
