//! Exit code of the EVM.

use edr_evm::HaltReason;

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
    /// Returns whether the exit code is an error.
    pub fn is_error(&self) -> bool {
        !matches!(self, Self::Success)
    }

    /// Returns whether the exit code is a contract too large error.
    pub fn is_contract_too_large_error(&self) -> bool {
        matches!(self, Self::Halt(HaltReason::CreateContractSizeLimit))
    }

    /// Returns whether the exit code is an invalid opcode error.
    pub fn is_invalid_opcode_error(&self) -> bool {
        matches!(
            self,
            Self::Halt(
                HaltReason::InvalidFEOpcode | HaltReason::OpcodeNotFound | HaltReason::NotActivated
            )
        )
    }

    /// Returns whether the exit code is an out of gas error.
    pub fn is_out_of_gas_error(&self) -> bool {
        matches!(self, Self::Halt(HaltReason::OutOfGas(_)))
    }

    /// Returns whether the exit code is a revert.
    pub fn is_revert(&self) -> bool {
        matches!(self, Self::Revert)
    }
}
