use std::sync::Arc;

use derive_more::Debug;
use dyn_clone::DynClone;
use edr_eth::HashMap;
use edr_evm::{
    inspector::Inspector,
    interpreter::{
        CallInputs, CallOutcome, CreateInputs, CreateOutcome, InstructionResult, InterpreterTypes,
    },
    journal::JournalTrait,
    spec::ContextTrait,
};
use edr_evm_spec::Transaction;
use edr_solidity::contract_decoder::{ContractDecoder, ContractIdentifierAndFunctionSignature};
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

#[derive(Clone, Debug)]
pub struct GasReport {
    pub contracts: HashMap<String, ContractGasReport>,
}

#[derive(Clone, Debug)]
pub struct GasReporter {
    pub collector: GasReportCollector,
    #[debug(skip)]
    callback: Box<dyn SyncOnCollectedGasReportCallback>,
}

impl GasReporter {
    pub fn new(
        callback: Box<dyn SyncOnCollectedGasReportCallback>,
        contract_decoder: Arc<ContractDecoder>,
    ) -> Self {
        Self {
            collector: GasReportCollector::new(contract_decoder),
            callback,
        }
    }

    pub fn report(self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let reports = self.collector.reports;
        (self.callback)(GasReport { contracts: reports })
    }
}

#[derive(Clone, Debug)]
pub struct GasReportCollector {
    pub reports: HashMap<String, ContractGasReport>,
    pub contract_decoder: Arc<ContractDecoder>,
}

impl GasReportCollector {
    pub fn new(contract_decoder: Arc<ContractDecoder>) -> Self {
        Self {
            reports: HashMap::new(),
            contract_decoder,
        }
    }
}

#[derive(Clone, Debug)]
pub struct ContractGasReport {
    pub deployments: Vec<DeploymentGasReport>,
    pub functions: HashMap<String, FunctionGasReport>,
}

#[derive(Clone, Debug)]
pub enum GasReportExecutionStatus {
    Success,
    Revert,
    Halt,
}

#[derive(Clone, Debug)]
pub struct DeploymentGasReport {
    pub gas: u64,
    pub size: u64,
    pub status: GasReportExecutionStatus,
}

#[derive(Clone, Debug)]
pub struct FunctionGasReport {
    pub calls: Vec<FunctionCallGasReport>,
}

#[derive(Clone, Debug)]
pub struct FunctionCallGasReport {
    pub gas: u64,
    pub status: GasReportExecutionStatus,
}

impl<ContextT: ContextTrait, InterpreterT: InterpreterTypes> Inspector<ContextT, InterpreterT>
    for GasReportCollector
{
    fn call_end(&mut self, context: &mut ContextT, inputs: &CallInputs, outcome: &mut CallOutcome) {
        if let TxKind::Call(to) = context.tx().kind() {
            if let Ok(code) = context.journal_mut().code(to) {
                let code = code.data;
                let input = inputs.input.bytes(context);

                let ContractIdentifierAndFunctionSignature {
                    contract_identifier,
                    function_signature,
                } = self
                    .contract_decoder
                    .get_contract_indentifier_and_function_singature_for_call(&code, Some(&input));

                let entry =
                    self.reports
                        .entry(contract_identifier)
                        .or_insert_with(|| ContractGasReport {
                            deployments: Vec::new(),
                            functions: HashMap::new(),
                        });
                if let Some(function_signature) = function_signature {
                    let result = *outcome.instruction_result();
                    let status = GasReportExecutionStatus::from(result);
                    let gas = outcome.gas().used();

                    entry
                        .functions
                        .entry(function_signature)
                        .or_insert_with(|| FunctionGasReport { calls: Vec::new() })
                        .calls
                        .push(FunctionCallGasReport { gas, status });
                }
            }
        }
    }

    fn create_end(
        &mut self,
        _context: &mut ContextT,
        inputs: &CreateInputs,
        outcome: &mut CreateOutcome,
    ) {
        let code = &inputs.init_code;

        let ContractIdentifierAndFunctionSignature {
            contract_identifier,
            ..
        } = self
            .contract_decoder
            .get_contract_indentifier_and_function_singature_for_call(code, None);

        let entry = self
            .reports
            .entry(contract_identifier)
            .or_insert_with(|| ContractGasReport {
                deployments: Vec::new(),
                functions: HashMap::new(),
            });
        let result = *outcome.instruction_result();
        let status = GasReportExecutionStatus::from(result);
        let gas = outcome.gas().used();
        let size = outcome.output().len() as u64;

        entry
            .deployments
            .push(DeploymentGasReport { gas, size, status });
    }
}

impl From<InstructionResult> for GasReportExecutionStatus {
    fn from(value: InstructionResult) -> Self {
        match value {
            InstructionResult::Stop
            | InstructionResult::Return
            | InstructionResult::SelfDestruct
            | InstructionResult::InvalidEOFInitCode => Self::Success,
            InstructionResult::Revert | InstructionResult::CreateInitCodeStartingEF00 => {
                Self::Revert
            }
            InstructionResult::CallTooDeep
            | InstructionResult::OutOfFunds
            | InstructionResult::OutOfGas
            | InstructionResult::MemoryLimitOOG
            | InstructionResult::MemoryOOG
            | InstructionResult::PrecompileOOG
            | InstructionResult::InvalidOperandOOG
            | InstructionResult::ReentrancySentryOOG
            | InstructionResult::OpcodeNotFound
            | InstructionResult::CallNotAllowedInsideStatic
            | InstructionResult::StateChangeDuringStaticCall
            | InstructionResult::InvalidFEOpcode
            | InstructionResult::InvalidJump
            | InstructionResult::NotActivated
            | InstructionResult::StackUnderflow
            | InstructionResult::StackOverflow
            | InstructionResult::OutOfOffset
            | InstructionResult::CreateCollision
            | InstructionResult::OverflowPayment
            | InstructionResult::PrecompileError
            | InstructionResult::NonceOverflow
            | InstructionResult::CreateContractSizeLimit
            | InstructionResult::CreateContractStartingWithEF
            | InstructionResult::CreateInitCodeSizeLimit
            | InstructionResult::FatalExternalError
            | InstructionResult::InvalidExtDelegateCallTarget => Self::Halt,
        }
    }
}
