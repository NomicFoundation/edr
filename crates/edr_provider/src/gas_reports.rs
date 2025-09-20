use derive_more::Debug;
use dyn_clone::DynClone;
use edr_eth::{hash_map, Address, Bytecode, Bytes, HashMap};
use edr_evm::state::{State, StateError};
use edr_evm_spec::HaltReasonTrait;
use edr_receipt::ExecutionResult;
use edr_solidity::{
    contract_decoder::{ContractDecoder, ContractIdentifierAndFunctionSignature},
    solidity_stack_trace::{UNRECOGNIZED_CONTRACT_NAME, UNRECOGNIZED_FUNCTION_NAME},
};
use edr_transaction::TxKind;

pub trait SyncOnCollectedGasReportCallback:
    Fn(GasReport) -> Result<(), Box<dyn std::error::Error + Send + Sync>> + DynClone + Send + Sync
{
}

impl<F> SyncOnCollectedGasReportCallback for F where
    F: Fn(GasReport) -> Result<(), Box<dyn std::error::Error + Send + Sync>>
        + DynClone
        + Send
        + Sync
{
}

dyn_clone::clone_trait_object!(SyncOnCollectedGasReportCallback);

#[derive(Clone, Debug, Default)]
pub struct GasReport {
    contracts: HashMap<String, ContractGasReport>,
}

/// An error that can occur when calling [`GasReport::new`] or
/// [`GasReport::add`].
#[derive(Debug, thiserror::Error)]
pub enum GasReportCreationError {
    /// Error caused by the state.
    #[error(transparent)]
    State(#[from] StateError),
}

impl GasReport {
    /// Creates a new instance with a single entry, based on the provided
    /// transaction parameters and execution result.
    ///
    /// If the contract or function could not be recognized, an empty report
    /// will be returned.
    pub fn new<HaltReasonT: HaltReasonTrait>(
        state: &dyn State<Error = StateError>,
        contract_decoder: &ContractDecoder,
        execution_result: &ExecutionResult<HaltReasonT>,
        kind: TxKind,
        input: Bytes,
    ) -> Result<Self, GasReportCreationError> {
        let mut report = GasReport::default();
        report.add(state, contract_decoder, execution_result, kind, input)?;
        Ok(report)
    }

    /// Consumes this instance and returns the inner map.
    pub fn into_inner(self) -> HashMap<String, ContractGasReport> {
        self.contracts
    }

    /// Adds a new entry to this instance based on the provided transaction
    /// parameters and execution result.
    ///
    /// If the contract or function could not be recognized, an empty report
    /// will be returned.
    pub fn add<HaltReasonT: HaltReasonTrait>(
        &mut self,
        state: &dyn State<Error = StateError>,
        contract_decoder: &ContractDecoder,
        execution_result: &ExecutionResult<HaltReasonT>,
        kind: TxKind,
        input: Bytes,
    ) -> Result<(), GasReportCreationError> {
        match ContractGasReportAndIdentifier::new(
            state,
            contract_decoder,
            execution_result,
            kind,
            input,
        ) {
            Ok(ContractGasReportAndIdentifier {
                contract_identifier,
                report,
            }) => match self.contracts.entry(contract_identifier) {
                hash_map::Entry::Occupied(mut occupied_entry) => {
                    occupied_entry.get_mut().merge(report);
                }
                hash_map::Entry::Vacant(vacant_entry) => {
                    vacant_entry.insert(report);
                }
            },
            Err(
                ContractGasReportCreationError::UnrecognizedContract
                | ContractGasReportCreationError::UnrecognizedFunction,
            ) => {
                // Ignore contracts & functions we couldn't recognize for now
            }
            Err(ContractGasReportCreationError::State(state_error)) => {
                return Err(state_error.into());
            }
        }

        Ok(())
    }

    /// Combines this instance with another [`GasReport`].
    pub fn merge(&mut self, other: GasReport) {
        for (contract_name, report) in other.contracts {
            match self.contracts.entry(contract_name) {
                hash_map::Entry::Occupied(mut occupied_entry) => {
                    occupied_entry.get_mut().merge(report);
                }
                hash_map::Entry::Vacant(vacant_entry) => {
                    vacant_entry.insert(report);
                }
            }
        }
    }
}

#[derive(Clone, Debug)]
pub struct ContractGasReport {
    pub deployments: Vec<DeploymentGasReport>,
    pub functions: HashMap<String, Vec<FunctionGasReport>>,
}

impl ContractGasReport {
    /// Combines this instance with another [`ContractGasReport`].
    pub fn merge(&mut self, other: ContractGasReport) {
        self.deployments.extend(other.deployments);

        for (function_name, function_reports) in other.functions {
            match self.functions.entry(function_name) {
                hash_map::Entry::Occupied(mut occupied_entry) => {
                    occupied_entry.get_mut().extend(function_reports);
                }
                hash_map::Entry::Vacant(vacant_entry) => {
                    vacant_entry.insert(function_reports);
                }
            }
        }
    }
}

/// An error that can occur when creating a [`ContractGasReport`].
#[derive(Debug, thiserror::Error)]
pub enum ContractGasReportCreationError {
    /// Error caused by the state.
    #[error(transparent)]
    State(StateError),
    /// The contract could not be recognized.
    #[error("Unrecognized contract")]
    UnrecognizedContract,
    /// The function could not be recognized.
    #[error("Unrecognized function")]
    UnrecognizedFunction,
}

impl From<DeploymentGasReportCreationError> for ContractGasReportCreationError {
    fn from(value: DeploymentGasReportCreationError) -> Self {
        match value {
            DeploymentGasReportCreationError::UnrecognizedContract => {
                ContractGasReportCreationError::UnrecognizedContract
            }
        }
    }
}

impl From<FunctionGasReportCreationError> for ContractGasReportCreationError {
    fn from(value: FunctionGasReportCreationError) -> Self {
        match value {
            FunctionGasReportCreationError::State(e) => ContractGasReportCreationError::State(e),
            FunctionGasReportCreationError::UnrecognizedContract => {
                ContractGasReportCreationError::UnrecognizedContract
            }
            FunctionGasReportCreationError::UnrecognizedFunction => {
                ContractGasReportCreationError::UnrecognizedFunction
            }
        }
    }
}

pub struct ContractGasReportAndIdentifier {
    pub contract_identifier: String,
    pub report: ContractGasReport,
}

impl ContractGasReportAndIdentifier {
    /// Constructs a new instance.
    pub fn new<HaltReasonT: HaltReasonTrait>(
        state: &dyn State<Error = StateError>,
        contract_decoder: &ContractDecoder,
        execution_result: &ExecutionResult<HaltReasonT>,
        kind: TxKind,
        input: Bytes,
    ) -> Result<Self, ContractGasReportCreationError> {
        if let TxKind::Call(to) = kind {
            let FunctionGasReportAndIdentifiers {
                contract_identifier,
                function_signature,
                report,
            } = FunctionGasReportAndIdentifiers::new(
                state,
                contract_decoder,
                execution_result,
                to,
                input,
            )?;

            let report = ContractGasReport {
                deployments: Vec::new(),
                functions: HashMap::from([(function_signature, vec![report])]),
            };

            Ok(ContractGasReportAndIdentifier {
                contract_identifier,
                report,
            })
        } else {
            let DeploymentGasReportAndIdentifiers {
                contract_identifier,
                report,
            } = DeploymentGasReportAndIdentifiers::new(contract_decoder, execution_result, input)?;

            let report = ContractGasReport {
                deployments: vec![report],
                functions: HashMap::new(),
            };

            Ok(ContractGasReportAndIdentifier {
                contract_identifier,
                report,
            })
        }
    }
}

#[derive(Clone, Debug)]
pub enum GasReportExecutionStatus {
    Success,
    Revert,
    Halt,
}

impl<HaltReasonT: HaltReasonTrait> From<&ExecutionResult<HaltReasonT>>
    for GasReportExecutionStatus
{
    fn from(result: &ExecutionResult<HaltReasonT>) -> Self {
        match result {
            ExecutionResult::Success { .. } => Self::Success,
            ExecutionResult::Revert { .. } => Self::Revert,
            ExecutionResult::Halt { .. } => Self::Halt,
        }
    }
}

#[derive(Clone, Debug)]
pub struct DeploymentGasReport {
    pub gas: u64,
    pub size: u64,
    pub status: GasReportExecutionStatus,
}

/// An error that can occur when creating a [`DeploymentGasReport`].
#[derive(Debug, thiserror::Error)]
pub enum DeploymentGasReportCreationError {
    /// The contract could not be recognized.
    #[error("Unrecognized contract")]
    UnrecognizedContract,
}

pub struct DeploymentGasReportAndIdentifiers {
    pub contract_identifier: String,
    pub report: DeploymentGasReport,
}

impl DeploymentGasReportAndIdentifiers {
    /// Constructs a new instance.
    pub fn new<HaltReasonT: HaltReasonTrait>(
        contract_decoder: &ContractDecoder,
        execution_result: &ExecutionResult<HaltReasonT>,
        code: Bytes,
    ) -> Result<Self, DeploymentGasReportCreationError> {
        let ContractIdentifierAndFunctionSignature {
            contract_identifier,
            ..
        } = contract_decoder.get_contract_identifier_and_function_signature_for_call(&code, None);

        if contract_identifier == UNRECOGNIZED_CONTRACT_NAME {
            return Err(DeploymentGasReportCreationError::UnrecognizedContract);
        }

        let report = DeploymentGasReport {
            gas: execution_result.gas_used(),
            size: code
                .len()
                .try_into()
                .expect("Contract code size should fit into u64"),
            status: execution_result.into(),
        };

        Ok(DeploymentGasReportAndIdentifiers {
            contract_identifier,
            report,
        })
    }
}

/// An error that can occur when creating a [`FunctionGasReport`].
#[derive(Debug, thiserror::Error)]
pub enum FunctionGasReportCreationError {
    /// Error caused by the state.
    #[error(transparent)]
    State(#[from] StateError),
    /// The contract could not be recognized.
    #[error("Unrecognized contract")]
    UnrecognizedContract,
    /// The function could not be recognized.
    #[error("Unrecognized function")]
    UnrecognizedFunction,
}

#[derive(Clone, Debug)]
pub struct FunctionGasReport {
    pub gas: u64,
    pub status: GasReportExecutionStatus,
}

pub struct FunctionGasReportAndIdentifiers {
    pub contract_identifier: String,
    pub function_signature: String,
    pub report: FunctionGasReport,
}

impl FunctionGasReportAndIdentifiers {
    /// Creates a new instance.
    pub fn new<HaltReasonT: HaltReasonTrait>(
        state: &dyn State<Error = StateError>,
        contract_decoder: &ContractDecoder,
        execution_result: &ExecutionResult<HaltReasonT>,
        to: Address,
        input: Bytes,
    ) -> Result<Self, FunctionGasReportCreationError> {
        let code = state
            .basic(to)?
            .map_or(Ok(Bytecode::default()), |account_info| {
                account_info
                    .code
                    .map_or_else(|| state.code_by_hash(account_info.code_hash), Ok)
            })?;

        let code = code.original_bytes();

        let ContractIdentifierAndFunctionSignature {
            contract_identifier,
            function_signature,
        } = contract_decoder
            .get_contract_identifier_and_function_signature_for_call(&code, Some(&input));

        if contract_identifier == UNRECOGNIZED_CONTRACT_NAME {
            return Err(FunctionGasReportCreationError::UnrecognizedContract);
        }

        if let Some(function_signature) = function_signature {
            if function_signature == UNRECOGNIZED_FUNCTION_NAME || function_signature.is_empty() {
                return Err(FunctionGasReportCreationError::UnrecognizedFunction);
            }

            let report = FunctionGasReport {
                gas: execution_result.gas_used(),
                status: execution_result.into(),
            };

            Ok(FunctionGasReportAndIdentifiers {
                contract_identifier,
                function_signature,
                report,
            })
        } else {
            Err(FunctionGasReportCreationError::UnrecognizedFunction)
        }
    }
}
