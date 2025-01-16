use core::marker::PhantomData;
use std::rc::Rc;

use derive_where::derive_where;
use revm_interpreter::{
    interpreter::InstructionProvider, table::CustomInstruction, Instruction, InstructionResult,
};
pub use revm_interpreter::{Host, Interpreter, InterpreterTypes};

/// A trait for instructions that can be inspected.
pub trait InspectsInstruction {
    type InterpreterTypes: InterpreterTypes;

    /// Called before the instruction is executed.
    fn before_instruction(&self, interpreter: &mut Interpreter<Self::InterpreterTypes>);

    /// Called after the instruction is executed.
    fn after_instruction(&self, interpreter: &mut Interpreter<Self::InterpreterTypes>);
}

/// A wrapper around an instruction that can be inspected.
pub struct InspectableInstruction<HostT, InterpreterTypesT: InterpreterTypes> {
    inner: fn(&mut Interpreter<InterpreterTypesT>, &mut HostT),
}

impl<HostT, InterpreterTypesT> CustomInstruction
    for InspectableInstruction<HostT, InterpreterTypesT>
where
    HostT: Host + InspectsInstruction<InterpreterTypes = InterpreterTypesT>,
    InterpreterTypesT: InterpreterTypes,
{
    type Host = HostT;
    type Wire = InterpreterTypesT;

    fn exec(&self, interpreter: &mut Interpreter<Self::Wire>, host: &mut Self::Host) {
        // SAFETY: As the PC was already incremented we need to subtract 1 to preserve
        // the old Inspector behavior.
        interpreter.bytecode.relative_jump(-1);

        host.before_instruction(interpreter);
        if interpreter.control.instruction_result() != InstructionResult::Continue {
            return;
        }

        // Reset PC to previous value.
        interpreter.bytecode.relative_jump(1);

        // Execute instruction.
        (self.instruction)(interpreter, host);

        host.after_instruction(interpreter);
    }

    fn from_base(instruction: Instruction<Self::Wire, Self::Host>) -> Self {
        Self { inner: instruction }
    }
}

#[derive_where(Clone)]
pub struct InspectableInstructionProvider<HostT, InterpreterTypesT, ProviderT>
where
    HostT: Host + InspectsInstruction<InterpreterTypes = InterpreterTypesT>,
    InterpreterTypesT: InterpreterTypes,
    ProviderT: InstructionProvider<Host = HostT, WIRE = InterpreterTypesT>,
{
    instruction_table: Rc<[InspectableInstruction<HostT, InterpreterTypesT>; 256]>,
    phantom: PhantomData<ProviderT>,
}

impl<HostT, InterpreterTypesT, ProviderT> InstructionProvider
    for InspectableInstructionProvider<HostT, InterpreterTypesT, ProviderT>
where
    HostT: Host + InspectsInstruction<InterpreterTypes = InterpreterTypesT>,
    InterpreterTypesT: InterpreterTypes,
    ProviderT: InstructionProvider<Host = HostT, WIRE = InterpreterTypesT>,
{
    type Host = HostT;
    type WIRE = InterpreterTypesT;

    fn new(context: &mut Self::Host) -> Self {
        let provider = ProviderT::new(context);
        let instruction_table = provider
            .table()
            .iter()
            .map(|instruction| InspectableInstruction::from_base(*instruction))
            .collect::<Rc<[_; 256]>>();

        Self {
            instruction_table,
            phantom: PhantomData,
        }
    }

    fn table(&mut self) -> &[impl CustomInstruction<Host = Self::Host, Wire = Self::WIRE>; 256] {
        self.instruction_table.as_ref()
    }
}
