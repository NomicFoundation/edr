//! Enriches the [`NestedTrace`] with the resolved [`ContractMetadata`].

use std::{fmt::Debug, sync::Arc};

use alloy_dyn_abi::{DynSolValue, FunctionExt as _, JsonAbiExt};
use edr_chain_spec::HaltReasonTrait;
use edr_common::fmt::format_token;
use edr_decoder_revert::RevertDecoder;
use edr_defaults::SELECTOR_LEN;
use edr_primitives::{Address, Bytes, HashMap, HashSet, Selector};
use foundry_evm_traces::{
    decoder::default_return_data, CallTraceArena, DecodedCallData, DecodedCallTrace,
};
use itertools::Itertools as _;
use parking_lot::RwLock;
use revm_inspectors::tracing::types::CallTrace;

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

/// Provides trace decoding with mutable access.
pub trait NestedTraceDecoderMut<HaltReasonT: HaltReasonTrait> {
    /// Enriches the [`NestedTrace`] with the resolved [`ContractMetadata`].
    fn try_to_decode_nested_trace_mut(
        &mut self,
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
    contracts_identifier: ContractsIdentifier,
    revert_decoder: RevertDecoder,
}

impl ContractDecoder {
    /// Creates a new [`ContractDecoder`].
    pub fn new(config: &BuildInfoConfig) -> Result<Self, ContractDecoderError> {
        let mut contracts_identifier = ContractsIdentifier::default();
        let mut revert_decoder = RevertDecoder::default();

        for build_info in &config.build_infos {
            let bytecodes = create_models_and_decode_bytecodes(
                build_info.solc_version.clone(),
                &build_info.input,
                &build_info.output,
            )
            .map_err(|error| ContractDecoderError::Initialization(error.to_string()))?;

            for bytecode in bytecodes {
                if config.ignore_contracts == Some(true)
                    && bytecode.contract.read().name.starts_with("Ignored")
                {
                    continue;
                }

                // Add the contract's custom errors to the revert decoder
                bytecode
                    .contract
                    .read()
                    .custom_errors
                    .iter()
                    .for_each(|error| {
                        revert_decoder.push_error(error.abi().clone());
                    });

                contracts_identifier.add_bytecode(Arc::new(bytecode));
            }
        }

        Ok(Self {
            contracts_identifier,
            revert_decoder,
        })
    }

    /// Adds contract metadata to the decoder.
    pub fn add_contract_metadata(&mut self, bytecode: ContractMetadata) {
        // Add all custom errors to the revert decoder
        bytecode
            .contract
            .read()
            .custom_errors
            .iter()
            .for_each(|error| {
                self.revert_decoder.push_error(error.abi().clone());
            });

        self.contracts_identifier.add_bytecode(Arc::new(bytecode));
    }

    /// Returns the contract and function names for the provided calldata.
    pub fn get_contract_and_function_names_for_call(
        &mut self,
        code: &Bytes,
        calldata: Option<&Bytes>,
    ) -> ContractAndFunctionName {
        let ContractIdentifierAndFunctionSignature {
            contract_identifier,
            function_signature,
        } = self.get_contract_identifier_and_function_signature_for_call(code, calldata);

        let contract_name = contract_identifier
            .rsplit_once(':')
            .map_or(contract_identifier.clone(), |(_, name)| name.to_string());

        let function_name = function_signature.as_ref().map(|signature| {
            signature
                .split_once('(')
                .map_or(signature.clone(), |(name, _)| name.to_string())
        });

        ContractAndFunctionName {
            contract_name,
            function_name,
        }
    }

    /// Returns the contract indentifier and function signature for the provided
    /// calldata.
    pub fn get_contract_identifier_and_function_signature_for_call(
        &mut self,
        code: &Bytes,
        calldata: Option<&Bytes>,
    ) -> ContractIdentifierAndFunctionSignature {
        let is_create = calldata.is_none();
        let bytecode = {
            self.contracts_identifier
                .get_bytecode_for_call(code.as_ref(), is_create)
        };

        let contract = bytecode.map(|bytecode| bytecode.contract.clone());
        let contract = contract.as_ref().map(|c| c.read());

        let contract_identifier = contract.as_ref().map_or_else(
            || UNRECOGNIZED_CONTRACT_NAME.to_string(),
            |c| {
                c.location.file().map_or_else(
                    |_| UNRECOGNIZED_CONTRACT_NAME.to_string(),
                    |file| {
                        let source_name = &file.read().source_name;
                        format!("{}:{}", source_name, c.name)
                    },
                )
            },
        );

        if is_create {
            ContractIdentifierAndFunctionSignature {
                contract_identifier,
                function_signature: None,
            }
        } else {
            match contract {
                None => ContractIdentifierAndFunctionSignature {
                    contract_identifier,
                    function_signature: Some("".to_string()),
                },
                Some(contract) => {
                    let calldata = match calldata {
                        Some(calldata) => calldata,
                        None => {
                            unreachable!("calldata should be Some if is_create is false")
                        }
                    };

                    let selector = &calldata.get(..SELECTOR_LEN).unwrap_or(&calldata[..]);

                    let func = contract.get_function_from_selector(selector);

                    let function_signature = match func {
                        Some(func) => {
                            let function_name = match func.r#type {
                                ContractFunctionType::Fallback => {
                                    FALLBACK_FUNCTION_NAME.to_string()
                                }
                                ContractFunctionType::Receive => RECEIVE_FUNCTION_NAME.to_string(),
                                _ => func.name.clone(),
                            };
                            let function = alloy_json_abi::Function::try_from(&**func);
                            if let Ok(function) = function {
                                let inputs = function
                                    .inputs
                                    .iter()
                                    .map(|param| param.ty.clone())
                                    .collect::<Vec<_>>()
                                    .join(",");
                                format!("{function_name}({inputs})")
                            } else {
                                function_name
                            }
                        }
                        None => UNRECOGNIZED_FUNCTION_NAME.to_string(),
                    };

                    ContractIdentifierAndFunctionSignature {
                        contract_identifier,
                        function_signature: Some(function_signature),
                    }
                }
            }
        }
    }

    /// Populates the call trace arena with decoded call traces.
    ///
    /// This is done for a whole [`CallTraceArena`] to avoid locking the
    /// [`ContractsIdentifier`] multiple times.
    pub fn populate_call_trace_arena(
        &mut self,
        call_trace_arena: &mut CallTraceArena,
        address_to_executed_code: &HashMap<Address, Bytes>,
        precompile_addresses: &HashSet<Address>,
    ) -> Result<(), serde_json::Error> {
        for node in call_trace_arena.nodes_mut() {
            let call_trace = &mut node.trace;

            let decoded = if precompile_addresses.contains(&call_trace.address)
                && let Some(decoded) = foundry_evm_traces::decoder::precompiles::decode(call_trace)
            {
                decoded
            } else if call_trace.kind.is_any_create() {
                let contract_metadata = self
                    .contracts_identifier
                    .get_bytecode_for_call(&call_trace.data, true);

                let contract_identifier = contract_metadata
                    .map_or(UNRECOGNIZED_CONTRACT_NAME.to_string(), |metadata| {
                        metadata.contract.read().name.clone()
                    });

                DecodedCallTrace {
                    label: Some(contract_identifier),
                    ..DecodedCallTrace::default()
                }
            } else {
                let calldata = &call_trace.data;
                let code = address_to_executed_code
                    .get(&call_trace.address)
                    .unwrap_or_default();

                let contract_metadata =
                    self.contracts_identifier.get_bytecode_for_call(code, false);

                if let Some(contract_metadata) = contract_metadata {
                    if let Some(Ok(selector)) = calldata.get(..SELECTOR_LEN).map(Selector::try_from)
                    {
                        let contract = contract_metadata.contract.read();
                        let label = Some(contract.name.clone());
                        if let Some(function) =
                            contract.get_function_from_selector(selector.as_slice())
                        {
                            let abi = alloy_json_abi::Function::try_from(function.as_ref())?;

                            let args = if let Some(input_data) = calldata.get(SELECTOR_LEN..)
                                && let Ok(args) = abi.abi_decode_input(input_data)
                            {
                                args.iter()
                                    .map(|value| format_value(value, &contract.name))
                                    .collect()
                            } else {
                                Vec::new()
                            };

                            let call_data = Some(DecodedCallData {
                                signature: abi.signature(),
                                args,
                            });

                            let return_data = decode_function_output(
                                call_trace,
                                &abi,
                                &contract.name,
                                &self.revert_decoder,
                            );

                            DecodedCallTrace {
                                label,
                                return_data,
                                call_data,
                            }
                        } else {
                            let return_data = if !call_trace.success {
                                let revert_msg = self
                                    .revert_decoder
                                    .decode(&call_trace.output, call_trace.status);

                                if call_trace.output.is_empty()
                                    || revert_msg.contains("EvmError: Revert")
                                {
                                    Some(format!(
                                    "unrecognized function selector {selector} for contract {contract_name} ({contract_address}).",
                                    contract_name = contract.name,
                                    contract_address = call_trace.address,
                                ))
                                } else {
                                    Some(revert_msg)
                                }
                            } else {
                                None
                            };

                            DecodedCallTrace {
                                label,
                                return_data,
                                call_data: Some(DecodedCallData {
                                    signature: UNRECOGNIZED_FUNCTION_NAME.to_owned(),
                                    args: if calldata.is_empty() {
                                        Vec::new()
                                    } else {
                                        vec![calldata.to_string()]
                                    },
                                }),
                            }
                        }
                    } else {
                        DecodedCallTrace {
                            label: Some(UNRECOGNIZED_CONTRACT_NAME.to_string()),
                            return_data: default_return_data(call_trace, &self.revert_decoder),
                            call_data: if call_trace.data.is_empty() {
                                None
                            } else {
                                Some(DecodedCallData {
                                    signature: UNRECOGNIZED_FUNCTION_NAME.to_owned(),
                                    args: vec![call_trace.data.to_string()],
                                })
                            },
                        }
                    }
                } else {
                    DecodedCallTrace {
                        label: Some(UNRECOGNIZED_CONTRACT_NAME.to_string()),
                        return_data: default_return_data(call_trace, &self.revert_decoder),
                        call_data: if call_trace.data.is_empty() {
                            None
                        } else {
                            Some(DecodedCallData {
                                signature: "".to_owned(),
                                args: vec![call_trace.data.to_string()],
                            })
                        },
                    }
                }
            };

            call_trace.decoded = Some(Box::new(decoded));
        }
        Ok(())
    }
}

/// Decodes the function output from the call trace using the provided function
/// ABI and contract name.
fn decode_function_output(
    call_trace: &CallTrace,
    function: &alloy_json_abi::Function,
    contract_name: &str,
    revert_decoder: &RevertDecoder,
) -> Option<String> {
    if !call_trace.success {
        return default_return_data(call_trace, revert_decoder);
    }

    if let Ok(values) = function.abi_decode_output(&call_trace.output) {
        return Some(
            values
                .iter()
                .map(|value| format_value(value, contract_name))
                .format(", ")
                .to_string(),
        );
    }

    None
}

fn format_value(value: &DynSolValue, contract_name: &str) -> String {
    if let DynSolValue::Address(address) = value {
        format!("{contract_name}: [{address}]",)
    } else {
        format_token(value)
    }
}

impl<HaltReasonT: HaltReasonTrait> NestedTraceDecoder<HaltReasonT> for RwLock<ContractDecoder> {
    fn try_to_decode_nested_trace(
        &self,
        nested_trace: NestedTrace<HaltReasonT>,
    ) -> Result<NestedTrace<HaltReasonT>, ContractDecoderError> {
        self.write().try_to_decode_nested_trace_mut(nested_trace)
    }
}

impl<HaltReasonT: HaltReasonTrait> NestedTraceDecoderMut<HaltReasonT> for ContractDecoder {
    fn try_to_decode_nested_trace_mut(
        &mut self,
        nested_trace: NestedTrace<HaltReasonT>,
    ) -> Result<NestedTrace<HaltReasonT>, ContractDecoderError> {
        match nested_trace {
            precompile @ NestedTrace::Precompile(..) => Ok(precompile),
            // NOTE: The branches below are the same with the difference of `is_create`
            NestedTrace::Call(mut call) => {
                let is_create = false;

                let contract_meta = {
                    self.contracts_identifier
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

                        let result = match self.try_to_decode_nested_trace_mut(trace)? {
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

                        let result = match self.try_to_decode_nested_trace_mut(trace)? {
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

/// A contract identifier and a function signature in the contract.
pub struct ContractIdentifierAndFunctionSignature {
    /// The contract identifier path.
    pub contract_identifier: String,
    /// The function signature.
    pub function_signature: Option<String>,
}
