//! Naive rewrite of `hardhat-network/provider/vm/exit.ts` from Hardhat.
//! Used together with `VMTracer`.

use std::fmt;

use edr_evm::HaltReason;
use edr_evm::SuccessReason;

#[derive(Clone, Copy)]
pub enum ExitCode {
    Success,
    Revert,
    OutOfGas,
    InternalError,
    InvalidOpcode,
    StackUnderflow,
    CodesizeExceedsMaximum,
    CreateCollision,
    StaticStateChange,
}

impl ExitCode {
    pub fn is_error(&self) -> bool {
        !matches!(self, ExitCode::Success)
    }
}

impl fmt::Display for ExitCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ExitCode::Success => write!(f, "Success"),
            ExitCode::Revert => write!(f, "Reverted"),
            ExitCode::OutOfGas => write!(f, "Out of gas"),
            ExitCode::InternalError => write!(f, "Internal error"),
            ExitCode::InvalidOpcode => write!(f, "Invalid opcode"),
            ExitCode::StackUnderflow => write!(f, "Stack underflow"),
            ExitCode::CodesizeExceedsMaximum => write!(f, "Codesize exceeds maximum"),
            ExitCode::CreateCollision => write!(f, "Create collision"),
            ExitCode::StaticStateChange => write!(f, "Static state change"),
        }
    }
}

impl From<SuccessReason> for ExitCode {
    fn from(reason: SuccessReason) -> Self {
        match reason {
            SuccessReason::Stop => Self::Success,
            SuccessReason::Return => Self::Success,
            SuccessReason::SelfDestruct => Self::Success,
        }
    }
}

impl From<HaltReason> for ExitCode {
    fn from(halt: HaltReason) -> Self {
        match halt {
        HaltReason::OutOfGas(_) => Self::OutOfGas,
        HaltReason::OpcodeNotFound | HaltReason::InvalidFEOpcode
        // Returned when an opcode is not implemented for the hardfork
        | HaltReason::NotActivated
        => Self::InvalidOpcode,
        HaltReason::StackUnderflow => Self::StackUnderflow,
        HaltReason::CreateCollision => Self::CreateCollision,
        HaltReason::CreateContractSizeLimit => Self::CodesizeExceedsMaximum,
        _ => panic!("Unmatched EDR exceptional halt: {halt:?}"),
    }
    }
}
