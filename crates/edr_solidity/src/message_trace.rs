//! Naive Rust port of the `MessageTrace` et al. from Hardhat.

use std::{cell::RefCell, rc::Rc};

use edr_eth::{Address, Bytes, U256};
use edr_evm::HaltReason;

use crate::build_model::Bytecode;

/// Represents the exit code of the EVM.
#[derive(Clone, Debug)]
pub enum ExitCode {
    /// Execution was successful.
    Success,
    /// Execution was reverted.
    Revert,
    /// Indicates that the EVM has experienced an exceptional halt.
    Halt(HaltReason),
}

impl ExitCode {
    pub fn is_error(&self) -> bool {
        !matches!(self, Self::Success)
    }

    pub fn is_contract_too_large_error(&self) -> bool {
        matches!(self, Self::Halt(HaltReason::CreateContractSizeLimit))
    }

    pub fn is_invalid_opcode_error(&self) -> bool {
        matches!(
            self,
            Self::Halt(
                HaltReason::InvalidFEOpcode | HaltReason::OpcodeNotFound | HaltReason::NotActivated
            )
        )
    }

    pub fn is_out_of_gas_error(&self) -> bool {
        matches!(self, Self::Halt(HaltReason::OutOfGas(_)))
    }

    pub fn is_revert(&self) -> bool {
        matches!(self, Self::Revert)
    }
}

/// Represents a message trace. Naive Rust port of the `MessageTrace` from
/// Hardhat.
#[derive(Clone, Debug)]
pub enum MessageTrace {
    /// Represents a create message trace.
    Create(CreateMessageTrace),
    /// Represents a call message trace.
    Call(CallMessageTrace),
    /// Represents a precompile message trace.
    Precompile(PrecompileMessageTrace),
}

impl MessageTrace {
    /// Returns a reference to the the common fields of the message trace.
    pub fn base(&mut self) -> &mut BaseMessageTrace {
        match self {
            MessageTrace::Create(create) => &mut create.base.base,
            MessageTrace::Call(call) => &mut call.base.base,
            MessageTrace::Precompile(precompile) => &mut precompile.base,
        }
    }

    pub fn exit(&self) -> &ExitCode {
        match self {
            MessageTrace::Create(create) => &create.base.base.exit,
            MessageTrace::Call(call) => &call.base.base.exit,
            MessageTrace::Precompile(precompile) => &precompile.base.exit,
        }
    }
}

/// Represents a message trace. Naive Rust port of the `MessageTrace` from
/// Hardhat.
#[derive(Clone, Debug)]
pub enum MessageTraceRef<'a> {
    /// Represents a create message trace.
    Create(&'a CreateMessageTrace),
    /// Represents a call message trace.
    Call(&'a CallMessageTrace),
    /// Represents a precompile message trace.
    Precompile(PrecompileMessageTrace),
}

/// Represents the common fields of a message trace.
#[derive(Clone, Debug)]
pub struct BaseMessageTrace {
    /// Value of the message.
    pub value: U256,
    /// Return data buffer.
    pub return_data: Bytes,
    /// EVM exit code.
    pub exit: ExitCode,
    /// How much gas was used.
    pub gas_used: u64,
    /// Depth of the message.
    pub depth: usize,
}

/// Represents a precompile message trace.
#[derive(Clone, Debug)]
pub struct PrecompileMessageTrace {
    /// Common fields of the message trace.
    pub base: BaseMessageTrace,
    /// Precompile number.
    pub precompile: u32,
    /// Calldata buffer
    pub calldata: Bytes,
}

impl PrecompileMessageTrace {
    pub fn exit(&self) -> &ExitCode {
        &self.base.exit
    }

    pub fn return_data(&self) -> &Bytes {
        &self.base.return_data
    }
}

/// Represents a base EVM message trace.
#[derive(Clone, Debug)]
pub struct BaseEvmMessageTrace {
    /// Common fields of the message trace.
    pub base: BaseMessageTrace,
    /// Code of the contract that is being executed.
    pub code: Bytes,
    /// Children message traces.
    pub steps: Vec<VmTracerMessageTraceStep>,
    /// Resolved metadata of the contract that is being executed.
    /// Filled in the JS side by `ContractsIdentifier`.
    pub bytecode: Option<Rc<Bytecode>>,
    // The following is just an optimization: When processing this traces it's useful to know ahead
    // of time how many subtraces there are.
    /// Number of subtraces. Used to speed up the processing of the traces in
    /// JS.
    pub number_of_subtraces: u32,
}

/// Represents a create message trace.
#[derive(Clone, Debug)]
pub struct CreateMessageTrace {
    /// Common fields
    pub base: BaseEvmMessageTrace,
    /// Address of the deployed contract.
    pub deployed_contract: Option<Bytes>,
}

impl CreateMessageTrace {
    /// Returns a reference to the metadata of the contract that is being
    /// executed.
    pub fn bytecode(&self) -> Option<&Rc<Bytecode>> {
        self.base.bytecode.as_ref()
    }

    pub fn set_bytecode(&mut self, bytecode: Option<Rc<Bytecode>>) {
        self.base.bytecode = bytecode
    }

    pub fn code(&self) -> &Bytes {
        &self.base.code
    }

    pub fn depth(&self) -> usize {
        self.base.base.depth
    }

    pub fn exit(&self) -> &ExitCode {
        &self.base.base.exit
    }

    pub fn number_of_subtraces(&self) -> u32 {
        self.base.number_of_subtraces
    }

    pub fn return_data(&self) -> &Bytes {
        &self.base.base.return_data
    }

    // TODO avoid clone
    pub fn steps(&self) -> Vec<MessageTraceStep> {
        self.base
            .steps
            .iter()
            .cloned()
            .map(MessageTraceStep::from)
            .collect()
    }

    // TODO avoid conversion
    pub fn set_steps(&mut self, steps: impl IntoIterator<Item = MessageTraceStep>) {
        self.base.steps = steps
            .into_iter()
            .map(VmTracerMessageTraceStep::from)
            .collect();
    }

    pub fn value(&self) -> &U256 {
        &self.base.base.value
    }
}

/// Represents a call message trace.
#[derive(Clone, Debug)]
pub struct CallMessageTrace {
    /// Common fields
    pub base: BaseEvmMessageTrace,
    /// Calldata buffer
    pub calldata: Bytes,
    /// Address of the contract that is being executed.
    pub address: Address,
    /// Address of the code that is being executed.
    pub code_address: Address,
}

impl CallMessageTrace {
    /// Returns a reference to the metadata of the contract that is being
    /// executed.
    pub fn bytecode(&self) -> Option<&Rc<Bytecode>> {
        self.base.bytecode.as_ref()
    }

    pub fn set_bytecode(&mut self, bytecode: Option<Rc<Bytecode>>) {
        self.base.bytecode = bytecode
    }

    pub fn code(&self) -> &Bytes {
        &self.base.code
    }

    pub fn depth(&self) -> usize {
        self.base.base.depth
    }

    pub fn exit(&self) -> &ExitCode {
        &self.base.base.exit
    }

    pub fn number_of_subtraces(&self) -> u32 {
        self.base.number_of_subtraces
    }

    pub fn return_data(&self) -> &Bytes {
        &self.base.base.return_data
    }

    // TODO avoid clone
    pub fn steps(&self) -> Vec<MessageTraceStep> {
        self.base
            .steps
            .iter()
            .cloned()
            .map(MessageTraceStep::from)
            .collect()
    }

    // TODO avoid conversion
    pub fn set_steps(&mut self, steps: impl IntoIterator<Item = MessageTraceStep>) {
        self.base.steps = steps
            .into_iter()
            .map(VmTracerMessageTraceStep::from)
            .collect();
    }

    pub fn value(&self) -> &U256 {
        &self.base.base.value
    }
}

/// Represents a message trace step. Naive Rust port of the `MessageTraceStep`
/// from Hardhat.
#[derive(Clone, Debug)]
pub enum VmTracerMessageTraceStep {
    /// [`MessageTrace`] variant.
    // It's both read and written to (updated) by the `VmTracer`.
    Message(Rc<RefCell<MessageTrace>>),
    /// [`EvmStep`] variant.
    Evm(EvmStep),
}

pub enum MessageTraceStep {
    /// Represents a create message trace.
    Create(CreateMessageTrace),
    /// Represents a call message trace.
    Call(CallMessageTrace),
    /// Represents a precompile message trace.
    Precompile(PrecompileMessageTrace),
    /// Minimal EVM step that contains only PC (program counter).
    Evm(EvmStep),
}

impl From<VmTracerMessageTraceStep> for MessageTraceStep {
    fn from(step: VmTracerMessageTraceStep) -> Self {
        match step {
            // TODO avoid clone
            VmTracerMessageTraceStep::Message(trace) => match trace.as_ref().borrow().clone() {
                MessageTrace::Create(create_trace) => MessageTraceStep::Create(create_trace),
                MessageTrace::Call(call_trace) => MessageTraceStep::Call(call_trace),
                MessageTrace::Precompile(precompile_trace) => {
                    MessageTraceStep::Precompile(precompile_trace)
                }
            },
            VmTracerMessageTraceStep::Evm(evm_step) => MessageTraceStep::Evm(evm_step),
        }
    }
}

impl From<MessageTraceStep> for VmTracerMessageTraceStep {
    fn from(step: MessageTraceStep) -> Self {
        match step {
            MessageTraceStep::Evm(evm_step) => VmTracerMessageTraceStep::Evm(evm_step),
            // message => VmTracerMessageTraceStep::Message(Rc::new(RefCell::new(message))),
            MessageTraceStep::Create(create) => VmTracerMessageTraceStep::Message(Rc::new(
                RefCell::new(MessageTrace::Create(create)),
            )),
            MessageTraceStep::Call(call) => {
                VmTracerMessageTraceStep::Message(Rc::new(RefCell::new(MessageTrace::Call(call))))
            }
            MessageTraceStep::Precompile(precompile) => VmTracerMessageTraceStep::Message(Rc::new(
                RefCell::new(MessageTrace::Precompile(precompile)),
            )),
        }
    }
}

/// Minimal EVM step that contains only PC (program counter).
#[derive(Clone, Debug)]
pub struct EvmStep {
    /// Program counter
    pub pc: u32,
}
