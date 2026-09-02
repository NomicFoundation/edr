//! Enriches the [`NestedTrace`] with the resolved `ContractMetadata`.

use std::fmt::Debug;

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
    build_model::ContractFunctionType,
    contracts_identifier::{ContractsIdentifier, IdentifiedContract},
    nested_trace::{NestedTrace, NestedTraceStep},
    proxy_detection::detect_proxy_chain,
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
    /// Enriches the [`NestedTrace`] with the resolved `ContractMetadata`.
    fn try_to_decode_nested_trace(
        &self,
        nested_trace: NestedTrace<HaltReasonT>,
    ) -> Result<NestedTrace<HaltReasonT>, ContractDecoderError>;
}

/// Provides trace decoding with mutable access.
pub trait NestedTraceDecoderMut<HaltReasonT: HaltReasonTrait> {
    /// Enriches the [`NestedTrace`] with the resolved `ContractMetadata`.
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
    pub fn new(config: BuildInfoConfig) -> Self {
        let mut contracts_identifier = ContractsIdentifier::default();
        let mut revert_decoder = RevertDecoder::default();

        for identified_contract in config.identified_contracts {
            if config.ignore_contracts == Some(true)
                && identified_contract
                    .contract_metadata
                    .contract
                    .read()
                    .name
                    .starts_with("Ignored")
            {
                continue;
            }

            // Add the contract's custom errors to the revert decoder
            identified_contract
                .contract_metadata
                .contract
                .read()
                .custom_errors
                .iter()
                .for_each(|error| {
                    revert_decoder.push_error(error.abi().clone());
                });

            contracts_identifier.add_bytecode(identified_contract);
        }

        Self {
            contracts_identifier,
            revert_decoder,
        }
    }

    /// Adds an identified contract (metadata + trace strategy) to the decoder.
    /// Used by the napi `hardhat_addCompilationResult` bridge.
    pub fn add_contract_metadata(&mut self, identified: IdentifiedContract) {
        // Add all custom errors to the revert decoder
        identified
            .contract_metadata
            .contract
            .read()
            .custom_errors
            .iter()
            .for_each(|error| {
                self.revert_decoder.push_error(error.abi().clone());
            });

        self.contracts_identifier.add_bytecode(identified);
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

        let contract = bytecode.map(|b| b.contract_metadata.contract.clone());
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
        // Decoding is done in two passes: the first pass computes the decoded
        // call traces with only immutable access to the arena, because calls
        // whose function selector is not found in the called contract's ABI
        // are resolved through proxy chain detection, which inspects other
        // nodes in the arena. The second pass assigns the results to the
        // nodes.
        let decoded_traces = {
            let arena: &CallTraceArena = call_trace_arena;
            arena
                .nodes()
                .iter()
                .enumerate()
                .map(|(node_idx, node)| {
                    self.decode_call_trace(
                        arena,
                        node_idx,
                        &node.trace,
                        address_to_executed_code,
                        precompile_addresses,
                    )
                })
                .collect::<Result<Vec<_>, serde_json::Error>>()?
        };

        for (node, decoded) in call_trace_arena.nodes_mut().iter_mut().zip(decoded_traces) {
            node.trace.decoded = Some(Box::new(decoded));
        }

        Ok(())
    }

    /// Decodes a single call trace of the arena, identifying the contract and
    /// the called function from the executed code.
    fn decode_call_trace(
        &mut self,
        call_trace_arena: &CallTraceArena,
        node_idx: usize,
        call_trace: &CallTrace,
        address_to_executed_code: &HashMap<Address, Bytes>,
        precompile_addresses: &HashSet<Address>,
    ) -> Result<DecodedCallTrace, serde_json::Error> {
        let decoded = if precompile_addresses.contains(&call_trace.address)
            && let Some(decoded) = foundry_evm_traces::decoder::precompiles::decode(call_trace)
        {
            decoded
        } else if call_trace.kind.is_any_create() {
            let identified = self
                .contracts_identifier
                .get_bytecode_for_call(&call_trace.data, true);

            let contract_identifier = identified
                .map_or(UNRECOGNIZED_CONTRACT_NAME.to_string(), |i| {
                    i.contract_metadata.contract.read().name.clone()
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

            let identified = self.contracts_identifier.get_bytecode_for_call(code, false);

            if let Some(identified) = identified {
                if let Some(Ok(selector)) = calldata.get(..SELECTOR_LEN).map(Selector::try_from) {
                    let contract = identified.contract_metadata.contract.read();
                    let label = Some(contract.name.clone());
                    if let Some(function) = contract.get_function_from_selector(selector.as_slice())
                    {
                        let abi = alloy_json_abi::Function::try_from(function.as_ref())?;

                        let args =
                            decode_input_args(&abi, calldata, &contract.name).unwrap_or_default();

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
                        // Try to resolve via proxy chain detection.
                        self.resolve_via_proxy_chain_or_unrecognized(
                            call_trace_arena,
                            node_idx,
                            call_trace,
                            &selector,
                            contract.name.clone(),
                            address_to_executed_code,
                        )?
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

        Ok(decoded)
    }

    /// Attempts to resolve a function selector via proxy chain detection.
    ///
    /// When a selector is not found in the called contract's ABI, this method
    /// checks if the call trace exhibits a proxy pattern (DELEGATECALL with
    /// matching selector). If so, it looks up the implementation contract's
    /// bytecode and tries to find the function in the implementation's ABI.
    ///
    /// Returns a [`DecodedCallTrace`] with:
    /// - The resolved function signature with proxy chain info if found via
    ///   proxy (e.g., "EIP173Proxy>GreetingsRegistry")
    /// - The unrecognized-selector fallback if not resolvable
    fn resolve_via_proxy_chain_or_unrecognized(
        &mut self,
        call_trace_arena: &CallTraceArena,
        node_idx: usize,
        call_trace: &CallTrace,
        selector: &Selector,
        contract_name: String,
        address_to_executed_code: &HashMap<Address, Bytes>,
    ) -> Result<DecodedCallTrace, serde_json::Error> {
        // `detect_proxy_chain` returns the chain ordered from the final
        // implementation to the outermost proxy.
        if let Some(proxy_chain) = detect_proxy_chain(call_trace_arena, node_idx)
            && let Some(implementation) = proxy_chain.first()
            && let Some(impl_code) = address_to_executed_code.get(&implementation.address)
            && let Some(impl_identified) = self
                .contracts_identifier
                .get_bytecode_for_call(impl_code, false)
        {
            let impl_contract = impl_identified.contract_metadata.contract.read();

            // Look up selector in implementation ABI
            if let Some(function) = impl_contract.get_function_from_selector(selector.as_slice()) {
                let abi = alloy_json_abi::Function::try_from(function.as_ref())?;

                // The proxy may forward modified calldata (e.g.
                // clones-with-immutable-args appends immutable arguments), so
                // fall back to the calldata that the implementation actually
                // received.
                let args = decode_input_args(&abi, &call_trace.data, &impl_contract.name)
                    .or_else(|| decode_input_args(&abi, &implementation.data, &impl_contract.name))
                    .unwrap_or_default();

                // Build the proxy chain label: "Proxy1>Proxy2>...>Implementation"
                // Start with the first contract name (already known)
                let chain_label = self.build_proxy_chain_label(
                    &contract_name,
                    &proxy_chain,
                    address_to_executed_code,
                );

                let call_data = Some(DecodedCallData {
                    signature: abi.signature(),
                    args,
                });

                let return_data = decode_function_output(
                    call_trace,
                    &abi,
                    &impl_contract.name,
                    &self.revert_decoder,
                );

                return Ok(DecodedCallTrace {
                    label: Some(chain_label),
                    return_data,
                    call_data,
                });
            }
        }

        // Fallback: selector not resolved via proxy chain
        let return_data = if !call_trace.success {
            let revert_msg = self
                .revert_decoder
                .decode(&call_trace.output, call_trace.status);

            if call_trace.output.is_empty() || revert_msg.contains("EvmError: Revert") {
                Some(format!(
                    "unrecognized function selector {selector} for contract {contract_name} ({contract_address}).",
                    contract_address = call_trace.address,
                ))
            } else {
                Some(revert_msg)
            }
        } else {
            None
        };

        Ok(DecodedCallTrace {
            label: Some(contract_name),
            return_data,
            call_data: Some(DecodedCallData {
                signature: UNRECOGNIZED_FUNCTION_NAME.to_owned(),
                args: if call_trace.data.is_empty() {
                    Vec::new()
                } else {
                    vec![call_trace.data.to_string()]
                },
            }),
        })
    }

    /// Builds a proxy chain label from a proxy chain ordered from the final
    /// implementation to the outermost proxy.
    ///
    /// Returns a string like "EIP173Proxy>Router>GreetingsRegistry" where each
    /// contract in the proxy chain is represented by its name, joined by `>`,
    /// starting with the outermost proxy.
    ///
    /// If a contract name cannot be resolved for an address, it falls back to
    /// using the address.
    fn build_proxy_chain_label(
        &mut self,
        outermost_contract_name: &str,
        proxy_chain: &[&CallTrace],
        address_to_executed_code: &HashMap<Address, Bytes>,
    ) -> String {
        let mut chain_names = vec![outermost_contract_name.to_string()];

        // Skip the outermost call, whose name is already known, and resolve
        // the rest.
        for call_trace in proxy_chain.iter().rev().skip(1) {
            let name = address_to_executed_code
                .get(&call_trace.address)
                .and_then(|code| self.contracts_identifier.get_bytecode_for_call(code, false))
                .map_or_else(
                    || format!("{:#x}", call_trace.address),
                    |identified| identified.contract_metadata.contract.read().name.clone(),
                );
            chain_names.push(name);
        }

        chain_names.join(">")
    }
}

/// Decodes the input arguments of the provided calldata using the function
/// ABI, formatting the values with the provided contract name.
///
/// Returns `None` if the calldata does not contain input data or if decoding
/// fails.
fn decode_input_args(
    function: &alloy_json_abi::Function,
    calldata: &Bytes,
    contract_name: &str,
) -> Option<Vec<String>> {
    let input_data = calldata.get(SELECTOR_LEN..)?;
    let args = function.abi_decode_input(input_data).ok()?;

    Some(
        args.iter()
            .map(|value| format_value(value, contract_name))
            .collect(),
    )
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

                let identified = self
                    .contracts_identifier
                    .get_bytecode_for_call(call.code.as_ref(), is_create);

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

                call.identified_contract = identified;
                call.steps = steps;

                Ok(NestedTrace::Call(call))
            }
            NestedTrace::Create(mut create @ CreateMessage { .. }) => {
                let is_create = true;

                let identified = self
                    .contracts_identifier
                    .get_bytecode_for_call(create.code.as_ref(), is_create);

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

                create.identified_contract = identified;
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
