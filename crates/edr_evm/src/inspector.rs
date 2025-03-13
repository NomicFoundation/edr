use std::{fmt::Debug, marker::PhantomData};

use edr_eth::{log::ExecutionLog, Address, U256};
use revm::{
    interpreter::{CallInputs, CreateInputs, Interpreter},
    Inspector,
};
use revm_interpreter::{CallOutcome, CreateOutcome, EOFCreateInputs, InterpreterTypes};

// TODO: Improve this design by introducing a InspectorMut trait

/// Inspector that allows two inspectors to operate side-by-side. The immutable
/// inspector runs first, followed by the mutable inspector. To ensure both
/// inspectors observe a valid state, you have to ensure that only the mutable
/// inspector modifies state. The returned values are solely determined by the
/// mutable inspector.
#[derive(Debug)]
pub struct DualInspector<A, B, ContextT, InterpreterT>
where
    A: Inspector<ContextT, InterpreterT>,
    B: Inspector<ContextT, InterpreterT>,
    InterpreterT: InterpreterTypes,
{
    immutable: A,
    mutable: B,
    phantom: PhantomData<(ContextT, InterpreterT)>,
}

impl<A, B, ContextT, InterpreterT> DualInspector<A, B, ContextT, InterpreterT>
where
    A: Inspector<ContextT, InterpreterT>,
    B: Inspector<ContextT, InterpreterT>,
    InterpreterT: InterpreterTypes,
{
    /// Constructs a `DualInspector` from the provided inspectors.
    pub fn new(immutable: A, mutable: B) -> Self {
        Self {
            immutable,
            mutable,
            phantom: PhantomData,
        }
    }

    /// Returns the two inspectors wrapped by the `DualInspector`.
    pub fn into_parts(self) -> (A, B) {
        (self.immutable, self.mutable)
    }
}

impl<A, B, ContextT, InterpreterT> Inspector<ContextT, InterpreterT>
    for DualInspector<A, B, ContextT, InterpreterT>
where
    A: Inspector<ContextT, InterpreterT>,
    B: Inspector<ContextT, InterpreterT>,
    InterpreterT: InterpreterTypes,
{
    fn initialize_interp(
        &mut self,
        interp: &mut Interpreter<InterpreterT>,
        context: &mut ContextT,
    ) {
        self.immutable.initialize_interp(interp, context);
        self.mutable.initialize_interp(interp, context);
    }

    fn step(&mut self, interp: &mut Interpreter<InterpreterT>, context: &mut ContextT) {
        self.immutable.step(interp, context);
        self.mutable.step(interp, context);
    }

    fn step_end(&mut self, interp: &mut Interpreter<InterpreterT>, context: &mut ContextT) {
        self.immutable.step_end(interp, context);
        self.mutable.step_end(interp, context);
    }

    fn log(
        &mut self,
        interp: &mut Interpreter<InterpreterT>,
        context: &mut ContextT,
        log: ExecutionLog,
    ) {
        self.immutable.log(interp, context, log.clone());
        self.mutable.log(interp, context, log);
    }

    fn call(&mut self, context: &mut ContextT, inputs: &mut CallInputs) -> Option<CallOutcome> {
        self.immutable.call(context, inputs);
        self.mutable.call(context, inputs)
    }

    fn call_end(&mut self, context: &mut ContextT, inputs: &CallInputs, outcome: &mut CallOutcome) {
        self.immutable.call_end(context, inputs, outcome);
        self.mutable.call_end(context, inputs, outcome)
    }

    fn create(
        &mut self,
        context: &mut ContextT,
        inputs: &mut CreateInputs,
    ) -> Option<CreateOutcome> {
        self.immutable.create(context, inputs);
        self.mutable.create(context, inputs)
    }

    fn create_end(
        &mut self,
        context: &mut ContextT,
        inputs: &CreateInputs,
        outcome: &mut CreateOutcome,
    ) {
        self.immutable.create_end(context, inputs, outcome);
        self.mutable.create_end(context, inputs, outcome)
    }

    fn eofcreate(
        &mut self,
        context: &mut ContextT,
        inputs: &mut EOFCreateInputs,
    ) -> Option<CreateOutcome> {
        self.immutable.eofcreate(context, inputs);
        self.mutable.eofcreate(context, inputs)
    }

    fn eofcreate_end(
        &mut self,
        context: &mut ContextT,
        inputs: &EOFCreateInputs,
        outcome: &mut CreateOutcome,
    ) {
        self.immutable.eofcreate_end(context, inputs, outcome);
        self.mutable.eofcreate_end(context, inputs, outcome)
    }

    fn selfdestruct(&mut self, contract: Address, target: Address, value: U256) {
        self.immutable.selfdestruct(contract, target, value);
        self.mutable.selfdestruct(contract, target, value);
    }
}
