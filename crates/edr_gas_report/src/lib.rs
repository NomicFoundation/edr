use derive_more::Debug;
use dyn_clone::DynClone;
use edr_chain_spec::HaltReasonTrait;
use edr_primitives::{
    hash_map::{self, HashMap},
    Address, Bytecode, Bytes,
};
use edr_receipt::{ExecutionResult, Output};
use edr_solidity::{
    contract_decoder::{ContractDecoder, ContractIdentifierAndFunctionSignature},
    proxy_detection::detect_proxy_chain,
    solidity_stack_trace::{UNRECOGNIZED_CONTRACT_NAME, UNRECOGNIZED_FUNCTION_NAME},
};
use edr_state_api::{State, StateError};
use edr_transaction::TxKind;
use revm_inspectors::tracing::CallTraceArena;

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
        contract_decoder: &mut ContractDecoder,
        execution_result: &ExecutionResult<HaltReasonT>,
        kind: TxKind,
        input: Bytes,
        call_trace_arena: &CallTraceArena,
    ) -> Result<Self, GasReportCreationError> {
        let mut report = GasReport::default();
        report.add(
            state,
            contract_decoder,
            execution_result,
            kind,
            input,
            call_trace_arena,
        )?;
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
        contract_decoder: &mut ContractDecoder,
        execution_result: &ExecutionResult<HaltReasonT>,
        kind: TxKind,
        input: Bytes,
        call_trace_arena: &CallTraceArena,
    ) -> Result<(), GasReportCreationError> {
        match ContractGasReportAndIdentifier::new(
            state,
            contract_decoder,
            execution_result,
            kind,
            input,
            call_trace_arena,
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
        contract_decoder: &mut ContractDecoder,
        execution_result: &ExecutionResult<HaltReasonT>,
        kind: TxKind,
        input: Bytes,
        call_trace_arena: &CallTraceArena,
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
                call_trace_arena,
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
    pub runtime_size: u64,
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
        contract_decoder: &mut ContractDecoder,
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

        let runtime_size = match execution_result {
            ExecutionResult::Success {
                output: Output::Create(bytes, _),
                ..
            } => bytes.len().try_into().unwrap_or_else(|_| {
                panic!(
                    "Length should be smaller than `u64::MAX`. Actual: {}",
                    bytes.len()
                )
            }),
            _ => 0,
        };

        let report = DeploymentGasReport {
            gas: execution_result.gas_used(),
            size: code
                .len()
                .try_into()
                .expect("Contract code size should fit into u64"),
            runtime_size,
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
    /// The proxy delegation chain for this call, if the called contract is a
    /// proxy. Contains contract identifiers from outermost proxy to final
    /// implementation, e.g. `["Proxy", "Implementation"]`.
    /// Empty if the call is not through a proxy.
    pub proxy_chain: Vec<String>,
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
        contract_decoder: &mut ContractDecoder,
        execution_result: &ExecutionResult<HaltReasonT>,
        to: Address,
        input: Bytes,
        call_trace_arena: &CallTraceArena,
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

            let proxy_chain =
                resolve_proxy_chain(call_trace_arena, state, contract_decoder).unwrap_or_default();

            let report = FunctionGasReport {
                gas: execution_result.gas_used(),
                status: execution_result.into(),
                proxy_chain,
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

/// Detects a proxy delegation chain from the call trace arena and resolves
/// each address to a contract name. Returns `None` if no proxy chain is
/// detected or if any address fails to resolve.
fn resolve_proxy_chain(
    arena: &CallTraceArena,
    state: &dyn State<Error = StateError>,
    contract_decoder: &mut ContractDecoder,
) -> Option<Vec<String>> {
    if arena.nodes().is_empty() {
        return None;
    }

    let chain_addrs = detect_proxy_chain(arena, 0);
    if chain_addrs.is_empty() {
        return None;
    }

    // All addresses must resolve
    chain_addrs
        .iter()
        .map(|addr| {
            // TODO: Instead of using the state, collect the bytecode for the addresses that
            // were called during execution. The usage of state to validate
            // whether code existed is fallible,because it's possible that
            // during execution of a transaction, the code field of an
            // address is overwritten.
            let code = state
                .basic(*addr)
                .ok()?
                .map_or(Ok(Bytecode::default()), |info| {
                    info.code
                        .map_or_else(|| state.code_by_hash(info.code_hash), Ok)
                })
                .ok()?;

            let ContractIdentifierAndFunctionSignature {
                contract_identifier,
                ..
            } = contract_decoder.get_contract_identifier_and_function_signature_for_call(
                &code.original_bytes(),
                None,
            );

            Some(contract_identifier)
        })
        .collect::<Option<Vec<_>>>()
}
