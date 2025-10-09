//! Types for the EVM interpreter.

pub use revm_interpreter::{
    interpreter::EthInterpreter,
    interpreter_types::{InputsTr, Jumps, LoopControl},
    return_revert, CallInputs, CallOutcome, CallValue, CreateInputs, CreateOutcome, FrameInput,
    Gas, Host, InputsImpl, InstructionResult, Interpreter, InterpreterResult, InterpreterTypes,
    SuccessOrHalt,
};
