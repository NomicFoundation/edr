//! Naive Rust port of the `MessageTrace` et al. from Hardhat.

use std::rc::Rc;

use edr_eth::{Address, Bytes, U256};

use crate::{build_model::ContractMetadata, exit_code::ExitCode};

/// An EVM trace where the steps are nested according to the call stack.
#[derive(Clone, Debug)]
pub enum NestedTrace {
    /// Represents a create trace.
    Create(CreateMessage),
    /// Represents a call trace.
    Call(CallMessage),
    /// Represents a precompile trace.
    Precompile(PrecompileMessage),
}

impl NestedTrace {
    /// Returns the exit code of the trace.
    pub fn exit_code(&self) -> &ExitCode {
        match self {
            Self::Create(create) => &create.exit,
            Self::Call(call) => &call.exit,
            Self::Precompile(precompile) => &precompile.exit,
        }
    }
}

/// Represents a precompile message.
#[derive(Clone, Debug)]
pub struct PrecompileMessage {
    /// Precompile number.
    pub precompile: u32,
    /// Calldata buffer
    pub calldata: Bytes,
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

/// Represents a create message.
#[derive(Clone, Debug)]
pub struct CreateMessage {
    // The following is just an optimization: When processing this traces it's useful to know ahead
    // of time how many subtraces there are.
    /// Number of subtraces. Used to speed up the processing of the traces in
    /// JS.
    pub number_of_subtraces: u32,
    /// Children messages.
    pub steps: Vec<NestedTraceStep>,
    /// Resolved metadata of the contract that is being executed.
    pub contract_meta: Option<Rc<ContractMetadata>>,
    /// Address of the deployed contract.
    pub deployed_contract: Option<Bytes>,
    /// Code of the contract that is being executed.
    pub code: Bytes,
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

/// Represents a call message with contract metadata.
#[derive(Clone, Debug)]
pub struct CallMessage {
    // The following is just an optimization: When processing this traces it's useful to know ahead
    // of time how many subtraces there are.
    /// Number of subtraces. Used to speed up the processing of the traces in
    /// JS.
    pub number_of_subtraces: u32,
    /// Children messages.
    pub steps: Vec<NestedTraceStep>,
    /// Resolved metadata of the contract that is being executed.
    pub contract_meta: Option<Rc<ContractMetadata>>,
    /// Calldata buffer
    pub calldata: Bytes,
    /// Address of the contract that is being executed.
    pub address: Address,
    /// Address of the code that is being executed.
    pub code_address: Address,
    /// Code of the contract that is being executed.
    pub code: Bytes,
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

/// Represents a create or call message.
#[derive(Clone, Debug)]
pub enum CreateOrCallMessage {
    /// Represents a create message.
    Create(CreateMessage),
    /// Represents a call message.
    Call(CallMessage),
}

impl From<CreateMessage> for CreateOrCallMessage {
    fn from(value: CreateMessage) -> Self {
        CreateOrCallMessage::Create(value)
    }
}

impl From<CallMessage> for CreateOrCallMessage {
    fn from(value: CallMessage) -> Self {
        CreateOrCallMessage::Call(value)
    }
}

/// Represents a create or call message.
#[derive(Clone, Copy, Debug)]
pub(crate) enum CreateOrCallMessageRef<'a> {
    /// Represents a create message.
    Create(&'a CreateMessage),
    /// Represents a call message.
    Call(&'a CallMessage),
}

impl<'a> CreateOrCallMessageRef<'a> {
    pub fn contract_meta(&self) -> Option<Rc<ContractMetadata>> {
        match self {
            CreateOrCallMessageRef::Create(create) => create.contract_meta.as_ref().map(Rc::clone),
            CreateOrCallMessageRef::Call(call) => call.contract_meta.as_ref().map(Rc::clone),
        }
    }

    pub fn exit_code(&self) -> &ExitCode {
        match self {
            CreateOrCallMessageRef::Create(create) => &create.exit,
            CreateOrCallMessageRef::Call(call) => &call.exit,
        }
    }

    pub fn number_of_subtraces(&self) -> u32 {
        match self {
            CreateOrCallMessageRef::Create(create) => create.number_of_subtraces,
            CreateOrCallMessageRef::Call(call) => call.number_of_subtraces,
        }
    }

    pub fn return_data(&self) -> &Bytes {
        match self {
            CreateOrCallMessageRef::Create(create) => &create.return_data,
            CreateOrCallMessageRef::Call(call) => &call.return_data,
        }
    }

    pub fn steps(&self) -> &'a [NestedTraceStep] {
        match self {
            CreateOrCallMessageRef::Create(create) => create.steps.as_slice(),
            CreateOrCallMessageRef::Call(call) => call.steps.as_slice(),
        }
    }
}

impl<'a> From<&'a CreateOrCallMessage> for CreateOrCallMessageRef<'a> {
    fn from(value: &'a CreateOrCallMessage) -> Self {
        match value {
            CreateOrCallMessage::Create(create) => CreateOrCallMessageRef::Create(create),
            CreateOrCallMessage::Call(call) => CreateOrCallMessageRef::Call(call),
        }
    }
}

impl<'a> From<&'a CreateMessage> for CreateOrCallMessageRef<'a> {
    fn from(value: &'a CreateMessage) -> Self {
        CreateOrCallMessageRef::Create(value)
    }
}

impl<'a> From<&'a CallMessage> for CreateOrCallMessageRef<'a> {
    fn from(value: &'a CallMessage) -> Self {
        CreateOrCallMessageRef::Call(value)
    }
}

/// Represents a nested trace step with contract metadata.
#[derive(Clone, Debug)]
pub enum NestedTraceStep {
    /// Represents a create message.
    Create(CreateMessage),
    /// Represents a call message.
    Call(CallMessage),
    /// Represents a precompile message.
    Precompile(PrecompileMessage),
    /// Minimal EVM step that contains only PC (program counter).
    Evm(EvmStep),
}

/// Minimal EVM step that contains only PC (program counter).
#[derive(Clone, Debug)]
pub struct EvmStep {
    /// Program counter
    pub pc: u32,
}
