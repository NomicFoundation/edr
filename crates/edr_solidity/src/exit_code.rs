//! Exit code of the EVM.

use edr_eth::l1;
use edr_evm_spec::HaltReasonTrait;

/// Represents the exit code of the EVM.
#[derive(Clone, Debug)]
pub enum ExitCode<HaltReasonT> {
    /// Execution was successful.
    Success,
    /// Execution was reverted.
    Revert,
    /// Indicates that the EVM has experienced an exceptional halt.
    Halt(HaltReasonT),
    /// A fatal external error that cannot be recovered from.
    FatalExternalError,
    /// An internal signal to continue execution.
    InternalContinue,
    /// Internal instruction that signals call or create.
    InternalCallOrCreate,
    /// Internal CREATE/CREATE starts with 0xEF00
    CreateInitCodeStartingEF00,
    /// Internal to `ExtDelegateCall`
    InvalidExtDelegateCallTarget,
}

impl<HaltReasonT> ExitCode<HaltReasonT> {
    /// Converts the type of the halt reason of the instance.
    pub fn map_halt_reason<ConversionFnT: Fn(HaltReasonT) -> NewHaltReasonT, NewHaltReasonT>(
        self,
        conversion_fn: ConversionFnT,
    ) -> ExitCode<NewHaltReasonT> {
        match self {
            ExitCode::Success => ExitCode::Success,
            ExitCode::Revert => ExitCode::Revert,
            ExitCode::Halt(reason) => ExitCode::Halt(conversion_fn(reason)),
            ExitCode::FatalExternalError => ExitCode::FatalExternalError,
            ExitCode::InternalContinue => ExitCode::InternalContinue,
            ExitCode::InternalCallOrCreate => ExitCode::InternalCallOrCreate,
            ExitCode::CreateInitCodeStartingEF00 => ExitCode::CreateInitCodeStartingEF00,
            ExitCode::InvalidExtDelegateCallTarget => ExitCode::InvalidExtDelegateCallTarget,
        }
    }
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
