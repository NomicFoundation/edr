use edr_eth::{Address, Bytes, U256};

use crate::{contracts_identifier::Bytecode, exit::ExitCode};

#[derive(Clone)]
pub enum MessageTrace {
    Create(CreateMessageTrace),
    Call(CallMessageTrace),
    Precompile(PrecompileMessageTrace),
}

impl MessageTrace {
    pub fn base(&mut self) -> &mut BaseMessageTrace {
        match self {
            MessageTrace::Create(create) => &mut create.base.base,
            MessageTrace::Call(call) => &mut call.base.base,
            MessageTrace::Precompile(precompile) => &mut precompile.base,
        }
    }
}

pub enum EvmMessageTrace {
    Create(CreateMessageTrace),
    Call(CallMessageTrace),
}

pub enum DecodedMessageTrace {
    Create(DecodedCreateMessageTrace),
    Call(DecodedCallMessageTrace),
}

#[derive(Clone)]
pub struct BaseMessageTrace {
    pub value: U256,
    pub return_data: Bytes,
    pub exit: ExitCode,
    pub gas_used: u64,
    pub depth: usize,
}

#[derive(Clone)]
pub struct PrecompileMessageTrace {
    pub base: BaseMessageTrace,
    pub precompile: u32,
    pub calldata: Bytes,
}

#[derive(Clone)]
pub struct BaseEvmMessageTrace {
    pub base: BaseMessageTrace,
    pub code: Bytes,
    pub steps: Vec<MessageTraceStep>,
    pub bytecode: Option<Bytecode>,
    // The following is just an optimization: When processing this traces it's useful to know ahead of
    // time how many subtraces there are.
    pub number_of_subtraces: u32,
}

#[derive(Clone)]
pub struct CreateMessageTrace {
    pub base: BaseEvmMessageTrace,
    pub deployed_contract: Option<Bytes>,
}

#[derive(Clone)]
pub struct CallMessageTrace {
    pub base: BaseEvmMessageTrace,
    pub calldata: Bytes,
    pub address: Address,
    pub code_address: Address,
}

pub struct DecodedCreateMessageTrace {
    pub base: CreateMessageTrace,
    pub bytecode: Bytecode,
}

pub struct DecodedCallMessageTrace {
    pub base: CallMessageTrace,
    pub bytecode: Bytecode,
}

pub fn is_precompile_trace(trace: &MessageTrace) -> bool {
    matches!(trace, MessageTrace::Precompile(_))
}

pub fn is_create_trace(trace: &MessageTrace) -> bool {
    matches!(trace, MessageTrace::Create(_)) && !is_call_trace(trace)
}

pub fn is_decoded_create_trace(trace: &MessageTrace) -> bool {
    if let MessageTrace::Create(create_trace) = trace {
        create_trace.base.bytecode.is_some()
    } else {
        false
    }
}

pub fn is_call_trace(trace: &MessageTrace) -> bool {
    matches!(trace, MessageTrace::Call(_))
}

pub fn is_decoded_call_trace(trace: &MessageTrace) -> bool {
    if let MessageTrace::Call(call_trace) = trace {
        call_trace.base.bytecode.is_some()
    } else {
        false
    }
}

pub fn is_evm_step(step: &MessageTraceStep) -> bool {
    matches!(step, MessageTraceStep::Evm(_))
}

#[derive(Clone)]
pub enum MessageTraceStep {
    Message(MessageTrace),
    Evm(EvmStep),
}

#[derive(Clone)]
pub struct EvmStep {
    pub pc: u64,
}
