use derive_more::Debug;
use dyn_clone::DynClone;
use edr_chain_spec::HaltReasonTrait;
use edr_primitives::{hash_map, Address, Bytes, HashMap};
use edr_receipt::{ExecutionResult, Output};
use edr_solidity::{
    contract_decoder::{ContractDecoder, ContractIdentifierAndFunctionSignature},
    proxy_detection::detect_proxy_chain,
    solidity_stack_trace::{UNRECOGNIZED_CONTRACT_NAME, UNRECOGNIZED_FUNCTION_NAME},
};
use edr_transaction::TxKind;
use revm_inspectors::tracing::{types::CallTrace, CallTraceArena};

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
    #[error("Missing code for contract at address {address}")]
    MissingCode { address: Address },
}

impl GasReport {
    /// Creates a new instance with a single entry, based on the provided
    /// transaction parameters and execution result.
    ///
    /// If the contract or function could not be recognized, an empty report
    /// will be returned.
    pub fn new<HaltReasonT: HaltReasonTrait>(
        contract_decoder: &mut ContractDecoder,
        execution_result: &ExecutionResult<HaltReasonT>,
        kind: TxKind,
        input: Bytes,
        call_trace_arena: &CallTraceArena,
        address_to_executed_code: &HashMap<Address, Bytes>,
    ) -> Result<Self, GasReportCreationError> {
        let mut report = GasReport::default();
        report.add(
            contract_decoder,
            execution_result,
            kind,
            input,
            call_trace_arena,
            address_to_executed_code,
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
        contract_decoder: &mut ContractDecoder,
        execution_result: &ExecutionResult<HaltReasonT>,
        kind: TxKind,
        input: Bytes,
        call_trace_arena: &CallTraceArena,
        address_to_executed_code: &HashMap<Address, Bytes>,
    ) -> Result<(), GasReportCreationError> {
        match ContractGasReportAndIdentifier::new(
            contract_decoder,
            execution_result,
            kind,
            input,
            call_trace_arena,
            address_to_executed_code,
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
            Err(ContractGasReportCreationError::MissingCode { address }) => {
                return Err(GasReportCreationError::MissingCode { address });
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
    #[error("Missing code for contract at address {address}")]
    MissingCode { address: Address },
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
            FunctionGasReportCreationError::MissingCode { address } => {
                ContractGasReportCreationError::MissingCode { address }
            }
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
        contract_decoder: &mut ContractDecoder,
        execution_result: &ExecutionResult<HaltReasonT>,
        kind: TxKind,
        input: Bytes,
        call_trace_arena: &CallTraceArena,
        address_to_executed_code: &HashMap<Address, Bytes>,
    ) -> Result<Self, ContractGasReportCreationError> {
        if let TxKind::Call(to) = kind {
            let FunctionGasReportAndIdentifiers {
                contract_identifier,
                function_signature,
                report,
            } = FunctionGasReportAndIdentifiers::new(
                contract_decoder,
                execution_result,
                to,
                input,
                call_trace_arena,
                address_to_executed_code,
            )?;

            let report = ContractGasReport {
                deployments: Vec::new(),
                functions: [(function_signature, vec![report])].into_iter().collect(),
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
                functions: HashMap::default(),
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
    #[error("Missing code for contract at address {address}")]
    MissingCode { address: Address },
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
        contract_decoder: &mut ContractDecoder,
        execution_result: &ExecutionResult<HaltReasonT>,
        to: Address,
        input: Bytes,
        call_trace_arena: &CallTraceArena,
        address_to_executed_code: &HashMap<Address, Bytes>,
    ) -> Result<Self, FunctionGasReportCreationError> {
        if let Some(proxy_chain) = detect_proxy_chain(call_trace_arena, 0) {
            match resolve_proxy_chain(
                contract_decoder,
                &input,
                execution_result,
                address_to_executed_code,
                proxy_chain,
            ) {
                Ok(gas_report) => return Ok(gas_report),
                Err(ResolveProxyChainError::EmptyProxyChain) => {
                    unreachable!("detect_proxy_chain should never return an empty chain")
                }
                Err(ResolveProxyChainError::MissingCode { address }) => {
                    return Err(FunctionGasReportCreationError::MissingCode { address });
                }
                Err(ResolveProxyChainError::UnrecognizedContract) => {
                    return Err(FunctionGasReportCreationError::UnrecognizedContract);
                }
                Err(ResolveProxyChainError::UnrecognizedFunction) => {
                    return Err(FunctionGasReportCreationError::UnrecognizedFunction);
                }
            }
        }

        let code = address_to_executed_code
            .get(&to)
            .cloned()
            .unwrap_or_default();

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
                proxy_chain: Vec::new(),
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

enum ResolveProxyChainError {
    EmptyProxyChain,
    MissingCode { address: Address },
    UnrecognizedContract,
    UnrecognizedFunction,
}

/// Tries to resolve the provided proxy chain to a
/// [`FunctionGasReportAndIdentifiers`].
///
/// The provided `proxy_chain` should be ordered from the final implementation
/// to the outermost proxy, e.g. `[implementation, proxyN, ..., proxy1]`.
///
/// The resolved proxy will contain the contract identifier and function
/// signature of the final implementation, while the used gas and execution
/// status will be taken from the original call (i.e. the outermost proxy). The
/// proxy chain in the returned `FunctionGasReport` will be ordered from the
/// outermost proxy to the final implementation, e.g. `[proxy1, proxyN, ...,
/// implementation]`.
fn resolve_proxy_chain<HaltReasonT: HaltReasonTrait>(
    contract_decoder: &mut ContractDecoder,
    input: &Bytes,
    execution_result: &ExecutionResult<HaltReasonT>,
    address_to_executed_code: &HashMap<Address, Bytes>,
    proxy_chain: Vec<&CallTrace>,
) -> Result<FunctionGasReportAndIdentifiers, ResolveProxyChainError> {
    let mut iter = proxy_chain.iter();
    let Some(implementation_call) = iter.next() else {
        return Err(ResolveProxyChainError::EmptyProxyChain);
    };

    let ContractIdentifierAndFunctionSignature {
        contract_identifier,
        function_signature,
    } = {
        let code = address_to_executed_code
            .get(&implementation_call.address)
            .ok_or(ResolveProxyChainError::MissingCode {
                address: implementation_call.address,
            })?;
        contract_decoder.get_contract_identifier_and_function_signature_for_call(code, Some(input))
    };

    if contract_identifier == UNRECOGNIZED_CONTRACT_NAME {
        return Err(ResolveProxyChainError::UnrecognizedContract);
    }

    let Some(function_signature) = function_signature else {
        return Err(ResolveProxyChainError::UnrecognizedFunction);
    };

    if function_signature == UNRECOGNIZED_FUNCTION_NAME || function_signature.is_empty() {
        return Err(ResolveProxyChainError::UnrecognizedFunction);
    }

    let proxy_chain = iter
        .rev()
        .map(|call_trace| {
            let code = address_to_executed_code.get(&call_trace.address).ok_or(
                ResolveProxyChainError::MissingCode {
                    address: call_trace.address,
                },
            )?;

            let contract_identifier = contract_decoder
                .get_contract_identifier_and_function_signature_for_call(code, None)
                .contract_identifier;

            Ok(contract_identifier)
        })
        .chain(std::iter::once(Ok(contract_identifier.clone())))
        .collect::<Result<Vec<String>, ResolveProxyChainError>>()?;

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
}
