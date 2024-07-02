//! Naive rewrite of `hardhat-network/provider/vm/exit.ts` from Hardhat.
//! Used together with `VmTracer`.

use std::fmt;

use edr_evm::HaltReason;

/// Represents the exit code of the EVM. Naive Rust port of the `ExitCode` from
/// Hardhat.
#[derive(Clone, Copy, Debug)]
pub enum ExitCode {
    /// Execution was successful.
    Success = 0,
    /// Execution was reverted.
    Revert,
    /// Execution ran out of gas.
    OutOfGas,
    /// Execution encountered an internal error.
    InternalError,
    /// Execution encountered an invalid opcode.
    InvalidOpcode,
    /// Execution encountered a stack underflow.
    StackUnderflow,
    /// Create init code size exceeds limit (runtime).
    CodesizeExceedsMaximum,
    /// Create collision.
    CreateCollision,
    /// Static state change.
    StaticStateChange,
}

impl TryFrom<u8> for ExitCode {
    type Error = &'static str;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(Self::Success),
            1 => Ok(Self::Revert),
            2 => Ok(Self::OutOfGas),
            3 => Ok(Self::InternalError),
            4 => Ok(Self::InvalidOpcode),
            5 => Ok(Self::StackUnderflow),
            6 => Ok(Self::CodesizeExceedsMaximum),
            7 => Ok(Self::CreateCollision),
            8 => Ok(Self::StaticStateChange),
            _ => Err("Invalid exit code"),
        }
    }
}

impl ExitCode {
    /// Whether the exit code represents an error.
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

#[allow(clippy::fallible_impl_from)] // naively ported for now
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
