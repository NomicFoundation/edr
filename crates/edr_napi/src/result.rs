use edr_chain_spec::EvmHaltReason;
use napi_derive::napi;

/// The possible reasons for successful termination of the EVM.
#[napi]
pub enum SuccessReason {
    /// The opcode `STOP` was called
    Stop,
    /// The opcode `RETURN` was called
    Return,
    /// The opcode `SELFDESTRUCT` was called
    SelfDestruct,
}

impl From<edr_chain_spec_evm::result::SuccessReason> for SuccessReason {
    fn from(eval: edr_chain_spec_evm::result::SuccessReason) -> Self {
        match eval {
            edr_chain_spec_evm::result::SuccessReason::Stop => Self::Stop,
            edr_chain_spec_evm::result::SuccessReason::Return => Self::Return,
            edr_chain_spec_evm::result::SuccessReason::SelfDestruct => Self::SelfDestruct,
        }
    }
}

impl From<SuccessReason> for edr_chain_spec_evm::result::SuccessReason {
    fn from(value: SuccessReason) -> Self {
        match value {
            SuccessReason::Stop => Self::Stop,
            SuccessReason::Return => Self::Return,
            SuccessReason::SelfDestruct => Self::SelfDestruct,
        }
    }
}

/// Indicates that the EVM has experienced an exceptional halt. This causes
/// execution to immediately end with all gas being consumed.
#[napi]
pub enum ExceptionalHalt {
    OutOfGas,
    OpcodeNotFound,
    InvalidFEOpcode,
    InvalidJump,
    NotActivated,
    StackUnderflow,
    StackOverflow,
    OutOfOffset,
    CreateCollision,
    PrecompileError,
    NonceOverflow,
    /// Create init code size exceeds limit (runtime).
    CreateContractSizeLimit,
    /// Error on created contract that begins with EF
    CreateContractStartingWithEF,
    /// EIP-3860: Limit and meter initcode. Initcode size limit exceeded.
    CreateInitCodeSizeLimit,
}

impl From<EvmHaltReason> for ExceptionalHalt {
    fn from(halt: EvmHaltReason) -> Self {
        match halt {
            EvmHaltReason::OutOfGas(..) => ExceptionalHalt::OutOfGas,
            EvmHaltReason::OpcodeNotFound => ExceptionalHalt::OpcodeNotFound,
            EvmHaltReason::InvalidFEOpcode => ExceptionalHalt::InvalidFEOpcode,
            EvmHaltReason::InvalidJump => ExceptionalHalt::InvalidJump,
            EvmHaltReason::NotActivated => ExceptionalHalt::NotActivated,
            EvmHaltReason::StackUnderflow => ExceptionalHalt::StackUnderflow,
            EvmHaltReason::StackOverflow => ExceptionalHalt::StackOverflow,
            EvmHaltReason::OutOfOffset => ExceptionalHalt::OutOfOffset,
            EvmHaltReason::CreateCollision => ExceptionalHalt::CreateCollision,
            EvmHaltReason::PrecompileError | EvmHaltReason::PrecompileErrorWithContext(_) => {
                ExceptionalHalt::PrecompileError
            }
            EvmHaltReason::NonceOverflow => ExceptionalHalt::NonceOverflow,
            EvmHaltReason::CreateContractSizeLimit => ExceptionalHalt::CreateContractSizeLimit,
            EvmHaltReason::CreateContractStartingWithEF => {
                ExceptionalHalt::CreateContractStartingWithEF
            }
            EvmHaltReason::CreateInitCodeSizeLimit => ExceptionalHalt::CreateInitCodeSizeLimit,
            EvmHaltReason::OverflowPayment
            | EvmHaltReason::StateChangeDuringStaticCall
            | EvmHaltReason::CallNotAllowedInsideStatic
            | EvmHaltReason::OutOfFunds
            | EvmHaltReason::CallTooDeep => {
                unreachable!("Internal halts that can be only found inside Inspector: {halt:?}")
            }
        }
    }
}
