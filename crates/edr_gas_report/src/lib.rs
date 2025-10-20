use std::fmt::Display;

use comfy_table::{presets::ASCII_MARKDOWN, Attribute, Cell, Color, Table};
use derive_more::Debug;
use dyn_clone::DynClone;
use edr_evm_spec::HaltReasonTrait;
use edr_primitives::{
    hash_map::{self, HashMap},
    Address, Bytecode, Bytes,
};
use edr_receipt::ExecutionResult;
use edr_solidity::{
    contract_decoder::{ContractDecoder, ContractIdentifierAndFunctionSignature},
    solidity_stack_trace::{UNRECOGNIZED_CONTRACT_NAME, UNRECOGNIZED_FUNCTION_NAME},
};
use edr_state_api::{State, StateError};
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
    pub contracts: HashMap<String, ContractGasReport>,
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

impl Display for GasReport {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> Result<(), std::fmt::Error> {
        // Sort contract names for consistent output
        let mut sorted_contracts: Vec<_> = self.contracts.iter().collect();
        sorted_contracts.sort_by(|a, b| a.0.cmp(b.0));

        for (name, contract) in sorted_contracts {
            if contract.deployments.is_empty() && contract.functions.is_empty() {
                continue;
            }

            let mut table = Table::new();
            table.load_preset(ASCII_MARKDOWN);
            table.set_header([Cell::new(format!("{name} contract"))
                .add_attribute(Attribute::Bold)
                .fg(Color::Green)]);

            // Add deployment information if available
            if !contract.deployments.is_empty() {
                table.add_row([
                    Cell::new("Deployment Cost")
                        .add_attribute(Attribute::Bold)
                        .fg(Color::Cyan),
                    Cell::new("Deployment Size")
                        .add_attribute(Attribute::Bold)
                        .fg(Color::Cyan),
                    Cell::new("Status")
                        .add_attribute(Attribute::Bold)
                        .fg(Color::Cyan),
                ]);

                for deployment in &contract.deployments {
                    let status_str = match deployment.status {
                        GasReportExecutionStatus::Success => "Success",
                        GasReportExecutionStatus::Revert => "Revert",
                        GasReportExecutionStatus::Halt => "Halt",
                    };
                    table.add_row([
                        Cell::new(deployment.gas.to_string()),
                        Cell::new(deployment.size.to_string()),
                        Cell::new(status_str).fg(match deployment.status {
                            GasReportExecutionStatus::Success => Color::Green,
                            GasReportExecutionStatus::Revert => Color::Yellow,
                            GasReportExecutionStatus::Halt => Color::Red,
                        }),
                    ]);
                }
            }

            // Add function information if available
            if !contract.functions.is_empty() {
                table.add_row([Cell::new("")]);
                table.add_row([
                    Cell::new("Function Name")
                        .add_attribute(Attribute::Bold)
                        .fg(Color::Magenta),
                    Cell::new("Gas")
                        .add_attribute(Attribute::Bold)
                        .fg(Color::Magenta),
                    Cell::new("Status")
                        .add_attribute(Attribute::Bold)
                        .fg(Color::Magenta),
                ]);

                // Sort function names for consistent output
                let mut sorted_functions: Vec<_> = contract.functions.iter().collect();
                sorted_functions.sort_by(|a, b| a.0.cmp(b.0));

                for (function_signature, reports) in sorted_functions {
                    if reports.is_empty() {
                        continue;
                    }

                    for report in reports {
                        let status_str = match report.status {
                            GasReportExecutionStatus::Success => "Success",
                            GasReportExecutionStatus::Revert => "Revert",
                            GasReportExecutionStatus::Halt => "Halt",
                        };
                        table.add_row([
                            Cell::new(function_signature.clone()).add_attribute(Attribute::Bold),
                            Cell::new(report.gas.to_string()).fg(Color::Yellow),
                            Cell::new(status_str).fg(match report.status {
                                GasReportExecutionStatus::Success => Color::Green,
                                GasReportExecutionStatus::Revert => Color::Yellow,
                                GasReportExecutionStatus::Halt => Color::Red,
                            }),
                        ]);
                    }
                }
            }

            writeln!(f, "{table}")?;
            writeln!(f)?;
        }
        Ok(())
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

#[derive(Clone, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum GasReportExecutionStatus {
    #[default]
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

        unsafe_fn();

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

use libc::size_t;

#[link(name = "snappy")]
unsafe extern "C" {
    fn snappy_max_compressed_length(source_length: size_t) -> size_t;
}

fn unsafe_fn() {
    let x = unsafe { snappy_max_compressed_length(100) };
    println!("max compressed length of a 100 byte buffer: {x}");
}
