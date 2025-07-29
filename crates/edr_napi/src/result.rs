use edr_evm::trace::AfterMessage;
use napi::{
    bindgen_prelude::{BigInt, Either3, Uint8Array},
    Either,
};
use napi_derive::napi;

use crate::log::ExecutionLog;

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

impl From<edr_eth::result::SuccessReason> for SuccessReason {
    fn from(eval: edr_eth::result::SuccessReason) -> Self {
        match eval {
            edr_eth::result::SuccessReason::Stop => Self::Stop,
            edr_eth::result::SuccessReason::Return => Self::Return,
            edr_eth::result::SuccessReason::SelfDestruct => Self::SelfDestruct,
        }
    }
}

impl From<SuccessReason> for edr_eth::result::SuccessReason {
    fn from(value: SuccessReason) -> Self {
        match value {
            SuccessReason::Stop => Self::Stop,
            SuccessReason::Return => Self::Return,
            SuccessReason::SelfDestruct => Self::SelfDestruct,
        }
    }
}

#[napi(object)]
pub struct CallOutput {
    /// Return value
    pub return_value: Uint8Array,
}

#[napi(object)]
pub struct CreateOutput {
    /// Return value
    pub return_value: Uint8Array,
    /// Optionally, a 160-bit address
    pub address: Option<Uint8Array>,
}

/// The result when the EVM terminates successfully.
#[napi(object)]
pub struct SuccessResult {
    /// The reason for termination
    pub reason: SuccessReason,
    /// The amount of gas used
    pub gas_used: BigInt,
    /// The amount of gas refunded
    pub gas_refunded: BigInt,
    /// The logs
    pub logs: Vec<ExecutionLog>,
    /// The transaction output
    pub output: Either<CallOutput, CreateOutput>,
}

/// The result when the EVM terminates due to a revert.
#[napi(object)]
pub struct RevertResult {
    /// The amount of gas used
    pub gas_used: BigInt,
    /// The transaction output
    pub output: Uint8Array,
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

impl From<edr_eth::l1::HaltReason> for ExceptionalHalt {
    fn from(halt: edr_eth::l1::HaltReason) -> Self {
        match halt {
            edr_eth::l1::HaltReason::OutOfGas(..) => ExceptionalHalt::OutOfGas,
            edr_eth::l1::HaltReason::OpcodeNotFound => ExceptionalHalt::OpcodeNotFound,
            edr_eth::l1::HaltReason::InvalidFEOpcode => ExceptionalHalt::InvalidFEOpcode,
            edr_eth::l1::HaltReason::InvalidJump => ExceptionalHalt::InvalidJump,
            edr_eth::l1::HaltReason::NotActivated => ExceptionalHalt::NotActivated,
            edr_eth::l1::HaltReason::StackUnderflow => ExceptionalHalt::StackUnderflow,
            edr_eth::l1::HaltReason::StackOverflow => ExceptionalHalt::StackOverflow,
            edr_eth::l1::HaltReason::OutOfOffset => ExceptionalHalt::OutOfOffset,
            edr_eth::l1::HaltReason::CreateCollision => ExceptionalHalt::CreateCollision,
            edr_eth::l1::HaltReason::PrecompileError => ExceptionalHalt::PrecompileError,
            edr_eth::l1::HaltReason::NonceOverflow => ExceptionalHalt::NonceOverflow,
            edr_eth::l1::HaltReason::CreateContractSizeLimit => {
                ExceptionalHalt::CreateContractSizeLimit
            }
            edr_eth::l1::HaltReason::CreateContractStartingWithEF => {
                ExceptionalHalt::CreateContractStartingWithEF
            }
            edr_eth::l1::HaltReason::CreateInitCodeSizeLimit => {
                ExceptionalHalt::CreateInitCodeSizeLimit
            }
            edr_eth::l1::HaltReason::OverflowPayment
            | edr_eth::l1::HaltReason::StateChangeDuringStaticCall
            | edr_eth::l1::HaltReason::CallNotAllowedInsideStatic
            | edr_eth::l1::HaltReason::OutOfFunds
            | edr_eth::l1::HaltReason::CallTooDeep => {
                unreachable!("Internal halts that can be only found inside Inspector: {halt:?}")
            }
        }
    }
}

impl From<ExceptionalHalt> for edr_eth::l1::HaltReason {
    fn from(value: ExceptionalHalt) -> Self {
        match value {
            ExceptionalHalt::OutOfGas => Self::OutOfGas(edr_eth::l1::OutOfGasError::Basic),
            ExceptionalHalt::OpcodeNotFound => Self::OpcodeNotFound,
            ExceptionalHalt::InvalidFEOpcode => Self::InvalidFEOpcode,
            ExceptionalHalt::InvalidJump => Self::InvalidJump,
            ExceptionalHalt::NotActivated => Self::NotActivated,
            ExceptionalHalt::StackUnderflow => Self::StackUnderflow,
            ExceptionalHalt::StackOverflow => Self::StackOverflow,
            ExceptionalHalt::OutOfOffset => Self::OutOfOffset,
            ExceptionalHalt::CreateCollision => Self::CreateCollision,
            ExceptionalHalt::PrecompileError => Self::PrecompileError,
            ExceptionalHalt::NonceOverflow => Self::NonceOverflow,
            ExceptionalHalt::CreateContractSizeLimit => Self::CreateContractSizeLimit,
            ExceptionalHalt::CreateContractStartingWithEF => Self::CreateContractStartingWithEF,
            ExceptionalHalt::CreateInitCodeSizeLimit => Self::CreateInitCodeSizeLimit,
        }
    }
}

/// The result when the EVM terminates due to an exceptional halt.
#[napi(object)]
pub struct HaltResult {
    /// The exceptional halt that occurred
    pub reason: ExceptionalHalt,
    /// Halting will spend all the gas and will thus be equal to the specified
    /// gas limit
    pub gas_used: BigInt,
}

/// The result of executing a transaction.
#[napi(object)]
pub struct ExecutionResult {
    /// The transaction result
    pub result: Either3<SuccessResult, RevertResult, HaltResult>,
    /// Optional contract address if the transaction created a new contract.
    pub contract_address: Option<Uint8Array>,
}

impl From<&AfterMessage<edr_eth::l1::HaltReason>> for ExecutionResult {
    fn from(value: &AfterMessage<edr_eth::l1::HaltReason>) -> Self {
        let AfterMessage {
            execution_result,
            contract_address,
        } = value;

        let result = match execution_result {
            edr_eth::result::ExecutionResult::Success {
                reason,
                gas_used,
                gas_refunded,
                logs,
                output,
            } => {
                let logs = logs.iter().map(ExecutionLog::from).collect();

                Either3::A(SuccessResult {
                    reason: SuccessReason::from(*reason),
                    gas_used: BigInt::from(*gas_used),
                    gas_refunded: BigInt::from(*gas_refunded),
                    logs,
                    output: match output {
                        edr_eth::result::Output::Call(return_value) => {
                            let return_value = Uint8Array::with_data_copied(return_value);

                            Either::A(CallOutput { return_value })
                        }
                        edr_eth::result::Output::Create(return_value, address) => {
                            let return_value = Uint8Array::with_data_copied(return_value);

                            Either::B(CreateOutput {
                                return_value,
                                address: address.as_ref().map(Uint8Array::with_data_copied),
                            })
                        }
                    },
                })
            }
            edr_eth::result::ExecutionResult::Revert { gas_used, output } => {
                let output = Uint8Array::with_data_copied(output);

                Either3::B(RevertResult {
                    gas_used: BigInt::from(*gas_used),
                    output,
                })
            }
            edr_eth::result::ExecutionResult::Halt { reason, gas_used } => Either3::C(HaltResult {
                reason: ExceptionalHalt::from(*reason),
                gas_used: BigInt::from(*gas_used),
            }),
        };

        let contract_address = contract_address.as_ref().map(Uint8Array::with_data_copied);

        Self {
            result,
            contract_address,
        }
    }
}
