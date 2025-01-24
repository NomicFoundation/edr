use core::marker::PhantomData;
use std::rc::Rc;

use derive_where::derive_where;
use revm_interpreter::{
    interpreter::InstructionProvider,
    interpreter_types::{Jumps as _, LoopControl as _},
    table::CustomInstruction,
    Instruction, InstructionResult,
};
pub use revm_interpreter::{Host, Interpreter, InterpreterTypes};

/// A trait for instructions that can be inspected.
pub trait InspectsInstruction {
    /// The type of interpreter-specific types.
    type InterpreterTypes: InterpreterTypes;

    /// Called before the instruction is executed.
    fn before_instruction(&mut self, interpreter: &Interpreter<Self::InterpreterTypes>);

    /// Called after the instruction is executed.
    fn after_instruction(&mut self, interpreter: &Interpreter<Self::InterpreterTypes>);
}

/// A trait for instructions that can be inspected with a provided journal.
pub trait InspectsInstructionWithJournal {
    /// The type of interpreter-specific types.
    type InterpreterTypes: InterpreterTypes;

    /// The type of journal.
    type Journal;

    /// Called before the instruction is executed.
    fn before_instruction_with_journal(
        &mut self,
        interpreter: &Interpreter<Self::InterpreterTypes>,
        journal: &Self::Journal,
    );

    /// Called after the instruction is executed.
    fn after_instruction_with_journal(
        &mut self,
        interpreter: &Interpreter<Self::InterpreterTypes>,
        journal: &Self::Journal,
    );
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
        (self.inner)(interpreter, host);

        host.after_instruction(interpreter);
    }

    fn from_base(instruction: Instruction<Self::Wire, Self::Host>) -> Self {
        Self { inner: instruction }
    }
}

impl<HostT, InterpreterT> From<Instruction<InterpreterT, HostT>>
    for InspectableInstruction<HostT, InterpreterT>
where
    InterpreterT: InterpreterTypes,
{
    fn from(inner: Instruction<InterpreterT, HostT>) -> Self {
        Self { inner }
    }
}

/// A provider for [`InspectableInstruction`]s.
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
    ProviderT: InstructionProvider<
        Host = HostT,
        Instruction: Clone + Into<InspectableInstruction<HostT, InterpreterTypesT>>,
        WIRE = InterpreterTypesT,
    >,
{
    type Host = HostT;
    type WIRE = InterpreterTypesT;
    type Instruction = InspectableInstruction<HostT, InterpreterTypesT>;

    fn new(context: &mut Self::Host) -> Self {
        let mut provider = ProviderT::new(context);

        let instruction_table = provider.table();
        debug_assert_eq!(instruction_table.len(), 256);

        let instruction_table =
            std::array::from_fn(|index| instruction_table[index].clone().into());

        Self {
            instruction_table: Rc::new(instruction_table),
            phantom: PhantomData,
        }
    }

    fn table(&mut self) -> &[Self::Instruction; 256] {
        self.instruction_table.as_ref()
    }
}
