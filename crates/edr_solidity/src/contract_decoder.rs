//! Enriches the [`NestedTrace`] with the resolved [`ContractMetadata`].

use std::{fmt::Debug, sync::Arc};

use edr_eth::Bytes;
use edr_evm_spec::HaltReasonTrait;
use parking_lot::RwLock;

use super::{
    nested_trace::CreateMessage,
    solidity_stack_trace::{
        FALLBACK_FUNCTION_NAME, RECEIVE_FUNCTION_NAME, UNRECOGNIZED_CONTRACT_NAME,
        UNRECOGNIZED_FUNCTION_NAME,
    },
};
use crate::{
    artifacts::BuildInfoConfig,
    build_model::{ContractFunctionType, ContractMetadata},
    compiler::create_models_and_decode_bytecodes,
    contracts_identifier::ContractsIdentifier,
    nested_trace::{NestedTrace, NestedTraceStep},
};

/// Errors that can occur during the decoding of the nested trace.
#[derive(Clone, Debug, thiserror::Error)]
pub enum ContractDecoderError {
    /// Errors that can occur when initializing the decoder.
    #[error("{0}")]
    Initialization(String),
}

/// Provides trace decoding
pub trait NestedTraceDecoder<HaltReasonT: HaltReasonTrait> {
    /// Enriches the [`NestedTrace`] with the resolved [`ContractMetadata`].
    fn try_to_decode_nested_trace(
        &self,
        nested_trace: NestedTrace<HaltReasonT>,
    ) -> Result<NestedTrace<HaltReasonT>, ContractDecoderError>;
}

/// `NestedTraceDecoder` with additional `Debug + Send + Sync` bounds.
pub trait SyncNestedTraceDecoder<HaltReasonT: HaltReasonTrait>:
    'static + NestedTraceDecoder<HaltReasonT> + Debug + Send + Sync
{
}

impl<HaltReasonT, T> SyncNestedTraceDecoder<HaltReasonT> for T
where
    HaltReasonT: HaltReasonTrait,
    T: 'static + NestedTraceDecoder<HaltReasonT> + Debug + Send + Sync,
{
}

/// Get contract metadata from calldata and traces.
#[derive(Debug, Default)]
pub struct ContractDecoder {
    contracts_identifier: RwLock<ContractsIdentifier>,
}

impl ContractDecoder {
    /// Creates a new [`ContractDecoder`].
    pub fn new(config: &BuildInfoConfig) -> Result<Self, ContractDecoderError> {
        let contracts_identifier = initialize_contracts_identifier(config)
            .map_err(|err| ContractDecoderError::Initialization(err.to_string()))?;
        Ok(Self {
            contracts_identifier: RwLock::new(contracts_identifier),
        })
    }

    /// Adds contract metadata to the decoder.
    pub fn add_contract_metadata(&self, bytecode: ContractMetadata) {
        self.contracts_identifier
            .write()
            .add_bytecode(Arc::new(bytecode));
    }

    /// Returns the contract and function names for the provided calldata.
    pub fn get_contract_and_function_names_for_call(
        &self,
        code: &Bytes,
        calldata: Option<&Bytes>,
    ) -> ContractAndFunctionName {
        let is_create = calldata.is_none();
        let bytecode = {
            self.contracts_identifier
                .write()
                .get_bytecode_for_call(code.as_ref(), is_create)
        };

        let contract = bytecode.map(|bytecode| bytecode.contract.clone());
        let contract = contract.as_ref().map(|c| c.read());

        let contract_name = contract.as_ref().map_or_else(
            || UNRECOGNIZED_CONTRACT_NAME.to_string(),
            |c| c.name.clone(),
        );

        if is_create {
            ContractAndFunctionName {
                contract_name,
                function_name: None,
            }
        } else {
            match contract {
                None => ContractAndFunctionName {
                    contract_name,
                    function_name: Some("".to_string()),
                },
                Some(contract) => {
                    let calldata = match calldata {
                        Some(calldata) => calldata,
                        None => {
                            unreachable!("calldata should be Some if is_create is false")
                        }
                    };

                    let selector = &calldata.get(..4).unwrap_or(&calldata[..]);

                    let func = contract.get_function_from_selector(selector);

                    let function_name = match func {
                        Some(func) => match func.r#type {
                            ContractFunctionType::Fallback => FALLBACK_FUNCTION_NAME.to_string(),
                            ContractFunctionType::Receive => RECEIVE_FUNCTION_NAME.to_string(),
                            _ => func.name.clone(),
                        },
                        None => UNRECOGNIZED_FUNCTION_NAME.to_string(),
                    };

                    ContractAndFunctionName {
                        contract_name,
                        function_name: Some(function_name),
                    }
                }
            }
        }
    }
}

impl<HaltReasonT: HaltReasonTrait> NestedTraceDecoder<HaltReasonT> for ContractDecoder {
    fn try_to_decode_nested_trace(
        &self,
        nested_trace: NestedTrace<HaltReasonT>,
    ) -> Result<NestedTrace<HaltReasonT>, ContractDecoderError> {
        match nested_trace {
            precompile @ NestedTrace::Precompile(..) => Ok(precompile),
            // NOTE: The branches below are the same with the difference of `is_create`
            NestedTrace::Call(mut call) => {
                let is_create = false;

                let contract_meta = {
                    self.contracts_identifier
                        .write()
                        .get_bytecode_for_call(call.code.as_ref(), is_create)
                };

                let steps = call
                    .steps
                    .into_iter()
                    .map(|step| {
                        let trace = match step {
                            NestedTraceStep::Evm(step) => return Ok(NestedTraceStep::Evm(step)),
                            NestedTraceStep::Precompile(precompile) => {
                                NestedTrace::Precompile(precompile)
                            }
                            NestedTraceStep::Create(create) => NestedTrace::Create(create),
                            NestedTraceStep::Call(call) => NestedTrace::Call(call),
                        };

                        let result = match self.try_to_decode_nested_trace(trace)? {
                            NestedTrace::Precompile(precompile) => {
                                NestedTraceStep::Precompile(precompile)
                            }
                            NestedTrace::Create(create) => NestedTraceStep::Create(create),
                            NestedTrace::Call(call) => NestedTraceStep::Call(call),
                        };

                        Ok(result)
                    })
                    .collect::<Result<Vec<_>, _>>()?;

                call.contract_meta = contract_meta;
                call.steps = steps;

                Ok(NestedTrace::Call(call))
            }
            NestedTrace::Create(mut create @ CreateMessage { .. }) => {
                let is_create = true;

                let contract_meta = {
                    self.contracts_identifier
                        .write()
                        .get_bytecode_for_call(create.code.as_ref(), is_create)
                };

                let steps = create
                    .steps
                    .into_iter()
                    .map(|step| {
                        let trace = match step {
                            NestedTraceStep::Evm(step) => return Ok(NestedTraceStep::Evm(step)),
                            NestedTraceStep::Precompile(precompile) => {
                                NestedTrace::Precompile(precompile)
                            }
                            NestedTraceStep::Create(create) => NestedTrace::Create(create),
                            NestedTraceStep::Call(call) => NestedTrace::Call(call),
                        };

                        let result = match self.try_to_decode_nested_trace(trace)? {
                            NestedTrace::Precompile(precompile) => {
                                NestedTraceStep::Precompile(precompile)
                            }
                            NestedTrace::Create(create) => NestedTraceStep::Create(create),
                            NestedTrace::Call(call) => NestedTraceStep::Call(call),
                        };

                        Ok(result)
                    })
                    .collect::<Result<Vec<_>, _>>()?;

                create.contract_meta = contract_meta;
                create.steps = steps;

                Ok(NestedTrace::Create(create))
            }
        }
    }
}

/// A contract and a function name in the contract.
pub struct ContractAndFunctionName {
    /// The name of the contract.
    pub contract_name: String,
    /// The name of the function.
    pub function_name: Option<String>,
}

fn initialize_contracts_identifier(
    config: &BuildInfoConfig,
) -> anyhow::Result<ContractsIdentifier> {
    let mut contracts_identifier = ContractsIdentifier::default();

    for build_info in &config.build_infos {
        let bytecodes = create_models_and_decode_bytecodes(
            build_info.solc_version.clone(),
            &build_info.input,
            &build_info.output,
        )?;

        for bytecode in bytecodes {
            if config.ignore_contracts == Some(true)
                && bytecode.contract.read().name.starts_with("Ignored")
            {
                continue;
            }

            contracts_identifier.add_bytecode(Arc::new(bytecode));
        }
    }

    Ok(contracts_identifier)
}
