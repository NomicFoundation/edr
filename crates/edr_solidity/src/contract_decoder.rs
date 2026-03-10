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
    proxy_function_resolver::{DecodedFunction, ProxyFunctionResolver, StateReader},
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

    /// Tries to resolve a function selector via ERC-1967 implementation slot.
    ///
    /// This method:
    /// 1. Reads the ERC-1967 implementation slot from the proxy address
    /// 2. Gets the bytecode at the implementation address
    /// 3. Matches the bytecode to a known contract
    /// 4. Looks up the selector in the matched contract's ABI
    ///
    /// Returns `Some(DecodedFunction)` with implementation info if found,
    /// `None` if ERC-1967 resolution fails or selector not found.
    pub fn try_resolve_selector_via_erc1967<S: StateReader + ?Sized>(
        &mut self,
        proxy_address: Address,
        selector: &[u8],
        state: &S,
    ) -> Option<DecodedFunction> {
        // Get the ERC-1967 implementation address
        let resolver = ProxyFunctionResolver::with_state(state);
        let impl_address = resolver.get_erc1967_implementation(proxy_address)?;

        // Get the bytecode at the implementation address
        let impl_bytecode = state.code(impl_address)?;

        // Match bytecode to known contract and lookup selector
        let (contract_metadata, signature) = self
            .contracts_identifier
            .search_selector_in_bytecode(&impl_bytecode, selector)?;

        let contract_name = contract_metadata.contract.read().name.clone();

        Some(DecodedFunction::from_implementation(
            signature,
            contract_name,
            impl_address,
        ))
    }

    /// Resolves a function selector for a potentially proxy contract.
    ///
    /// This method implements the resolution algorithm:
    /// 1. First, try to find the selector in the called contract's ABI (handled
    ///    by caller)
    /// 2. If not found, try ERC-1967 implementation slot detection
    /// 3. If still not found, fallback to searching all known contracts
    ///
    /// Returns a [`DecodedFunction`] with resolution info.
    pub fn resolve_proxy_selector<S: StateReader + ?Sized>(
        &mut self,
        proxy_address: Address,
        selector: &[u8],
        state: Option<&S>,
    ) -> DecodedFunction {
        // Step 1: Try ERC-1967 resolution if state is available
        if let Some(state) = state {
            if let Some(decoded) =
                self.try_resolve_selector_via_erc1967(proxy_address, selector, state)
            {
                return decoded;
            }
        }

        // Step 2: Fallback - search all known contracts
        let fallback_result = self
            .contracts_identifier
            .search_selector_in_all_contracts(selector);

        if let Some(signature) = fallback_result.format_signature() {
            DecodedFunction::from_fallback(signature)
        } else {
            DecodedFunction::unrecognized()
        }
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
                        None => {
                            // Selector not found in the called contract's ABI.
                            // Try fallback search across all known contracts.
                            // Note: We need to drop the contract lock before calling
                            // search_selector_in_all_contracts
                            drop(contract);
                            let fallback_result = self
                                .contracts_identifier
                                .search_selector_in_all_contracts(selector);

                            fallback_result
                                .format_signature()
                                .unwrap_or_else(|| UNRECOGNIZED_FUNCTION_NAME.to_string())
                        }
                    };

                    ContractIdentifierAndFunctionSignature {
                        contract_identifier,
                        function_signature: Some(function_signature),
                    }
                }
            }
        }
    }

    /// Returns the contract identifier and function signature for the provided
    /// calldata, using ERC-1967 implementation slot detection for proxy
    /// contracts.
    ///
    /// This method is similar to
    /// [`Self::get_contract_identifier_and_function_signature_for_call`] but
    /// also accepts an address and state reader for ERC-1967 proxy detection.
    ///
    /// When a selector is not found in the called contract's ABI:
    /// 1. First tries to resolve via ERC-1967 implementation slot
    /// 2. Falls back to searching all known contracts
    pub fn get_contract_identifier_and_function_signature_for_call_with_state<S: StateReader + ?Sized>(
        &mut self,
        code: &Bytes,
        calldata: Option<&Bytes>,
        address: Address,
        state: &S,
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
                        None => {
                            // Selector not found in the called contract's ABI.
                            // First, try ERC-1967 implementation slot detection.
                            // Note: We need to drop the contract lock before calling methods
                            // that may access the contracts_identifier
                            drop(contract);

                            let decoded = self.resolve_proxy_selector(address, selector, Some(state));
                            decoded.signature
                        }
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
                            // Selector not found in the called contract's ABI.
                            // Try fallback search across all known contracts.
                            let contract_name = contract.name.clone();
                            drop(contract);
                            let fallback_result = self
                                .contracts_identifier
                                .search_selector_in_all_contracts(selector.as_slice());

                            let decoded_function = if let Some(signature) =
                                fallback_result.format_signature()
                            {
                                DecodedFunction::from_fallback(signature)
                            } else {
                                DecodedFunction::unrecognized()
                            };

                            let return_data = if !call_trace.success {
                                let revert_msg = self
                                    .revert_decoder
                                    .decode(&call_trace.output, call_trace.status);

                                if call_trace.output.is_empty()
                                    || revert_msg.contains("EvmError: Revert")
                                {
                                    if decoded_function.signature == UNRECOGNIZED_FUNCTION_NAME {
                                        Some(format!(
                                            "unrecognized function selector {selector} for contract {contract_name} ({contract_address}).",
                                            contract_address = call_trace.address,
                                        ))
                                    } else {
                                        // Function was resolved via fallback, show that instead
                                        Some(format!(
                                            "call to {} reverted.",
                                            decoded_function.signature,
                                        ))
                                    }
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
                                    signature: decoded_function.signature,
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

    /// Populates the call trace arena with decoded call traces, using ERC-1967
    /// implementation slot detection for proxy contracts.
    ///
    /// This is similar to [`Self::populate_call_trace_arena`] but also accepts
    /// a state reader for ERC-1967 proxy detection. When a selector is not
    /// found in the called contract's ABI:
    /// 1. First tries to resolve via ERC-1967 implementation slot
    /// 2. Falls back to searching all known contracts
    pub fn populate_call_trace_arena_with_state<S: StateReader + ?Sized>(
        &mut self,
        call_trace_arena: &mut CallTraceArena,
        address_to_executed_code: &HashMap<Address, Bytes>,
        precompile_addresses: &HashSet<Address>,
        state: &S,
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
                            // Selector not found in the called contract's ABI.
                            // First, try ERC-1967 implementation slot detection, then fallback.
                            // Note: We need to drop the contract lock before calling
                            // resolve_proxy_selector
                            let contract_name = contract.name.clone();
                            drop(contract);

                            let decoded_function = self.resolve_proxy_selector(
                                call_trace.address,
                                selector.as_slice(),
                                Some(state),
                            );

                            let return_data = if !call_trace.success {
                                let revert_msg = self
                                    .revert_decoder
                                    .decode(&call_trace.output, call_trace.status);

                                if call_trace.output.is_empty()
                                    || revert_msg.contains("EvmError: Revert")
                                {
                                    if decoded_function.signature == UNRECOGNIZED_FUNCTION_NAME {
                                        Some(format!(
                                            "unrecognized function selector {selector} for contract {contract_name} ({contract_address}).",
                                            contract_address = call_trace.address,
                                        ))
                                    } else {
                                        // Function was resolved via ERC-1967 or fallback
                                        Some(format!(
                                            "call to {} reverted.",
                                            decoded_function.signature,
                                        ))
                                    }
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
                                    signature: decoded_function.signature,
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
