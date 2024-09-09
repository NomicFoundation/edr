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

/// Represents a base EVM message trace.
#[derive(Clone, Debug)]
pub struct BaseEvmMessageTrace {
    /// Common fields of the message trace.
    pub base: BaseMessageTrace,
    /// Code of the contract that is being executed.
    pub code: Bytes,
    /// Children message traces.
    pub steps: Vec<MessageTraceStep>,
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

/// Represents a message trace step. Naive Rust port of the `MessageTraceStep`
/// from Hardhat.
#[derive(Clone, Debug)]
pub enum MessageTraceStep {
    /// [`MessageTrace`] variant.
    // It's both read and written to (updated) by the `VmTracer`.
    Message(Rc<RefCell<MessageTrace>>),
    /// [`EvmStep`] variant.
    Evm(EvmStep),
}

/// Minimal EVM step that contains only PC (program counter).
#[derive(Clone, Debug)]
pub struct EvmStep {
    /// Program counter
    pub pc: u64,
}
