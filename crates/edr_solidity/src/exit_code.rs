//! Exit code of the EVM.

use edr_eth::{l1, spec::HaltReasonTrait};

/// Represents the exit code of the EVM.
#[derive(Clone, Debug)]
pub enum ExitCode<HaltReasonT: HaltReasonTrait> {
    /// Execution was successful.
    Success,
    /// Execution was reverted.
    Revert,
    /// Indicates that the EVM has experienced an exceptional halt.
    Halt(HaltReasonT),
}

impl<HaltReasonT: HaltReasonTrait> ExitCode<HaltReasonT> {
    /// Returns whether the exit code is an error.
    pub fn is_error(&self) -> bool {
        !matches!(self, Self::Success)
    }

    /// Returns whether the exit code is a contract too large error.
    pub fn is_contract_too_large_error(&self) -> bool {
        if let Self::Halt(reason) = self {
            *reason == l1::HaltReason::CreateContractSizeLimit.into()
        } else {
            false
        }
    }

    /// Returns whether the exit code is an invalid opcode error.
    pub fn is_invalid_opcode_error(&self) -> bool {
        if let Self::Halt(reason) = self {
            (*reason == l1::HaltReason::InvalidFEOpcode.into())
                | (*reason == l1::HaltReason::OpcodeNotFound.into())
                | (*reason == l1::HaltReason::NotActivated.into())
        } else {
            false
        }
    }

    /// Returns whether the exit code is an out of gas error.
    pub fn is_out_of_gas_error(&self) -> bool {
        if let Self::Halt(reason) = self {
            (*reason == l1::HaltReason::OutOfGas(l1::OutOfGasError::Basic).into())
                | (*reason == l1::HaltReason::OutOfGas(l1::OutOfGasError::MemoryLimit).into())
                | (*reason == l1::HaltReason::OutOfGas(l1::OutOfGasError::Memory).into())
                | (*reason == l1::HaltReason::OutOfGas(l1::OutOfGasError::Precompile).into())
                | (*reason == l1::HaltReason::OutOfGas(l1::OutOfGasError::InvalidOperand).into())
                | (*reason == l1::HaltReason::OutOfGas(l1::OutOfGasError::ReentrancySentry).into())
        } else {
            false
        }
    }

    /// Returns whether the exit code is a revert.
    pub fn is_revert(&self) -> bool {
        matches!(self, Self::Revert)
    }
}
