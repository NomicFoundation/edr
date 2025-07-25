use std::collections::{hash_map::Entry, BTreeMap, HashMap};

use alloy_dyn_abi::{DecodedEvent, DynSolValue, EventExt, FunctionExt, JsonAbiExt};
use alloy_json_abi::{Error, Event, Function, JsonAbi};
use alloy_primitives::{Address, LogData, Selector, B256};
use edr_defaults::SELECTOR_LEN;
use foundry_evm_core::{
    abi::{fmt::format_token, Console, HardhatConsole, Vm, HARDHAT_CONSOLE_SELECTOR_PATCHES},
    constants::{
        CALLER, CHEATCODE_ADDRESS, DEFAULT_CREATE2_DEPLOYER, HARDHAT_CONSOLE_ADDRESS,
        TEST_CONTRACT_ADDRESS,
    },
    contracts::ContractsByArtifact,
    decode::RevertDecoder,
};
use itertools::Itertools;
use once_cell::sync::OnceCell;
use revm_inspectors::tracing::types::{DecodedCallLog, DecodedCallTrace};
use rustc_hash::FxHashMap;

use crate::{
    abi::get_indexed_event,
    identifier::{
        AddressIdentity, LocalTraceIdentifier, SingleSignaturesIdentifier, TraceIdentifier,
    },
    CallTrace, CallTraceArena, CallTraceNode, DecodedCallData,
};

mod precompiles;

/// Build a new [`CallTraceDecoder`].
#[derive(Default)]
#[must_use = "builders do nothing unless you call `build` on them"]
pub struct CallTraceDecoderBuilder {
    decoder: CallTraceDecoder,
}

impl CallTraceDecoderBuilder {
    /// Create a new builder.
    #[inline]
    pub fn new() -> Self {
        Self {
            decoder: CallTraceDecoder::new().clone(),
        }
    }

    /// Add known labels to the decoder.
    #[inline]
    pub fn with_labels(mut self, labels: impl IntoIterator<Item = (Address, String)>) -> Self {
        self.decoder.labels.extend(labels);
        self
    }

    /// Add known errors to the decoder.
    #[inline]
    pub fn with_abi(mut self, abi: &JsonAbi) -> Self {
        self.decoder.collect_abi(abi, None);
        self
    }

    /// Add known contracts to the decoder.
    #[inline]
    pub fn with_known_contracts(mut self, contracts: &ContractsByArtifact) -> Self {
        trace!(target: "evm::traces", len=contracts.len(), "collecting known contract ABIs");
        for contract in contracts.values() {
            self.decoder.collect_abi(&contract.abi, None);
        }
        self
    }

    /// Add known contracts to the decoder from a `LocalTraceIdentifier`.
    #[inline]
    pub fn with_local_identifier_abis(self, identifier: &LocalTraceIdentifier<'_>) -> Self {
        self.with_known_contracts(identifier.contracts())
    }

    /// Sets the signature identifier for events and functions.
    #[inline]
    pub fn with_signature_identifier(mut self, identifier: SingleSignaturesIdentifier) -> Self {
        self.decoder.signature_identifier = Some(identifier);
        self
    }

    /// Build the decoder.
    #[inline]
    pub fn build(self) -> CallTraceDecoder {
        self.decoder
    }
}

/// The call trace decoder.
///
/// The decoder collects address labels and ABIs from any number of
/// [`TraceIdentifier`]s, which it then uses to decode the call trace.
///
/// Note that a call trace decoder is required for each new set of traces, since
/// addresses in different sets might overlap.
#[derive(Clone, Debug, Default)]
pub struct CallTraceDecoder {
    /// Addresses identified to be a specific contract.
    ///
    /// The values are in the form `"<artifact>:<contract>"`.
    pub contracts: HashMap<Address, String>,
    /// Address labels.
    pub labels: HashMap<Address, String>,
    /// Contract addresses that have a receive function.
    pub receive_contracts: Vec<Address>,

    /// All known functions.
    pub functions: FxHashMap<Selector, Vec<Function>>,
    /// All known events.
    pub events: BTreeMap<(B256, usize), Vec<Event>>,
    /// Revert decoder. Contains all known custom errors.
    pub revert_decoder: RevertDecoder,

    /// A signature identifier for events and functions.
    pub signature_identifier: Option<SingleSignaturesIdentifier>,
    /// Whether to include values in cheatcode decoding instead of placeholders.
    /// Since cheatcodes are used to load large files, values are hidden by
    /// default.
    pub verbose_cheatcode_decoding: bool,
}

impl CallTraceDecoder {
    /// Creates a new call trace decoder.
    ///
    /// The call trace decoder always knows how to decode calls to the cheatcode
    /// address, as well as DSTest-style logs.
    pub fn new() -> &'static Self {
        // If you want to take arguments in this function, assign them to the fields of
        // the cloned lazy instead of removing it
        static INIT: OnceCell<CallTraceDecoder> = OnceCell::new();
        INIT.get_or_init(Self::init)
    }

    fn init() -> Self {
        /// All functions from the Hardhat console ABI.
        ///
        /// See [`HARDHAT_CONSOLE_SELECTOR_PATCHES`] for more details.
        fn hh_funcs() -> impl Iterator<Item = (Selector, Function)> {
            let functions = HardhatConsole::abi::functions();
            let mut functions: Vec<_> = functions
                .into_values()
                .flatten()
                .map(|func| (func.selector(), func))
                .collect();
            let len = functions.len();
            // `functions` is the list of all patched functions; duplicate the unpatched
            // ones
            for (unpatched, patched) in HARDHAT_CONSOLE_SELECTOR_PATCHES.iter() {
                if let Some((_, func)) = functions[..len].iter().find(|(sel, _)| sel == patched) {
                    functions.push((unpatched.into(), func.clone()));
                }
            }
            functions.into_iter()
        }

        Self {
            contracts: HashMap::default(),
            labels: [
                (CHEATCODE_ADDRESS, "VM".to_string()),
                (HARDHAT_CONSOLE_ADDRESS, "console".to_string()),
                (DEFAULT_CREATE2_DEPLOYER, "Create2Deployer".to_string()),
                (CALLER, "DefaultSender".to_string()),
                (TEST_CONTRACT_ADDRESS, "DefaultTestContract".to_string()),
            ]
            .into(),
            receive_contracts: Vec::default(),

            functions: hh_funcs()
                .chain(
                    Vm::abi::functions()
                        .into_values()
                        .flatten()
                        .map(|func| (func.selector(), func)),
                )
                .map(|(selector, func)| (selector, vec![func]))
                .collect(),
            events: Console::abi::events()
                .into_values()
                .flatten()
                .map(|event| ((event.selector(), indexed_inputs(&event)), vec![event]))
                .collect(),
            revert_decoder: RevertDecoder::default(),

            signature_identifier: None,
            verbose_cheatcode_decoding: false,
        }
    }

    /// Clears all known addresses.
    pub fn clear_addresses(&mut self) {
        self.contracts.clear();

        let default_labels = &Self::new().labels;
        if self.labels.len() > default_labels.len() {
            self.labels.clone_from(default_labels);
        }

        self.receive_contracts.clear();
    }

    /// Identify unknown addresses in the specified call trace using the
    /// specified identifier.
    ///
    /// Unknown contracts are contracts that either lack a label or an ABI.
    pub fn identify(&mut self, trace: &CallTraceArena, identifier: &mut impl TraceIdentifier) {
        self.collect_identities(identifier.identify_addresses(self.trace_addresses(trace)));
    }

    /// Adds a single event to the decoder.
    pub fn push_event(&mut self, event: Event) {
        self.events
            .entry((event.selector(), indexed_inputs(&event)))
            .or_default()
            .push(event);
    }

    /// Adds a single function to the decoder.
    pub fn push_function(&mut self, function: Function) {
        match self.functions.entry(function.selector()) {
            Entry::Occupied(entry) => {
                // This shouldn't happen that often.
                if entry.get().contains(&function) {
                    return;
                }
                trace!(target: "evm::traces", selector=%entry.key(), new=%function.signature(), "duplicate function selector");
                entry.into_mut().push(function);
            }
            Entry::Vacant(entry) => {
                entry.insert(vec![function]);
            }
        }
    }

    /// Adds a single error to the decoder.
    pub fn push_error(&mut self, error: Error) {
        self.revert_decoder.push_error(error);
    }

    /// Returns an iterator over the trace addresses.
    pub fn trace_addresses<'a>(
        &'a self,
        arena: &'a CallTraceArena,
    ) -> impl Iterator<Item = (&'a Address, Option<&'a [u8]>)> + Clone + 'a {
        arena
            .nodes()
            .iter()
            .map(|node| {
                (
                    &node.trace.address,
                    node.trace
                        .kind
                        .is_any_create()
                        .then_some(&node.trace.output[..]),
                )
            })
            .filter(|(address, _)| {
                !self.labels.contains_key(*address) || !self.contracts.contains_key(*address)
            })
    }

    fn collect_identities(&mut self, identities: Vec<AddressIdentity<'_>>) {
        // Skip logging if there are no identities.
        if identities.is_empty() {
            return;
        }

        trace!(target: "evm::traces", len=identities.len(), "collecting address identities");
        for AddressIdentity {
            address,
            label,
            contract,
            abi,
            artifact_id: _,
        } in identities
        {
            let _span = trace_span!(target: "evm::traces", "identity", ?contract, ?label).entered();

            if let Some(contract) = contract {
                self.contracts.entry(address).or_insert(contract);
            }

            if let Some(label) = label {
                self.labels.entry(address).or_insert(label);
            }

            if let Some(abi) = abi {
                self.collect_abi(&abi, Some(&address));
            }
        }
    }

    fn collect_abi(&mut self, abi: &JsonAbi, address: Option<&Address>) {
        trace!(target: "evm::traces", len=abi.len(), ?address, "collecting ABI");
        for function in abi.functions() {
            self.push_function(function.clone());
        }
        for event in abi.events() {
            self.push_event(event.clone());
        }
        for error in abi.errors() {
            self.push_error(error.clone());
        }
        if let Some(address) = address {
            if abi.receive.is_some() {
                self.receive_contracts.push(*address);
            }
        }
    }

    /// Populates the traces with decoded data by mutating the
    /// [`CallTrace`] in place. See [`CallTraceDecoder::decode_function`] and
    /// [`CallTraceDecoder::decode_event`] for more details.
    pub async fn populate_traces(&self, traces: &mut Vec<CallTraceNode>) {
        for node in traces {
            node.trace.decoded = self.decode_function(&node.trace).await;
            for log in node.logs.iter_mut() {
                log.decoded = self.decode_event(&log.raw_log).await;
            }
        }
    }

    /// Decodes a call trace.
    pub async fn decode_function(&self, trace: &CallTrace) -> DecodedCallTrace {
        if let Some(trace) = precompiles::decode(trace, 1) {
            return trace;
        }

        let label = self.labels.get(&trace.address).cloned();

        let cdata = &trace.data;
        if trace.address == DEFAULT_CREATE2_DEPLOYER {
            return DecodedCallTrace {
                label,
                call_data: Some(DecodedCallData {
                    signature: "create2".to_string(),
                    args: vec![],
                }),
                return_data: (!trace.status.is_ok()).then(|| {
                    self.revert_decoder
                        .decode(&trace.output, Some(trace.status))
                }),
            };
        }

        if cdata.len() >= SELECTOR_LEN {
            let selector = &cdata[..SELECTOR_LEN];
            let mut functions = Vec::new();
            // The Clippy suggestion makes the code more difficult to read in this case.
            #[allow(clippy::single_match_else)]
            let functions = match self.functions.get(selector) {
                Some(fs) => fs,
                None => {
                    if let Some(identifier) = &self.signature_identifier {
                        if let Some(function) =
                            identifier.write().await.identify_function(selector).await
                        {
                            functions.push(function);
                        }
                    }
                    &functions
                }
            };
            let [func, ..] = &functions[..] else {
                return DecodedCallTrace {
                    label,
                    call_data: None,
                    return_data: None,
                };
            };

            DecodedCallTrace {
                label,
                call_data: Some(self.decode_function_input(trace, func)),
                return_data: self.decode_function_output(trace, functions),
            }
        } else {
            let has_receive = self.receive_contracts.contains(&trace.address);
            let signature = if cdata.is_empty() && has_receive {
                "receive()"
            } else {
                "fallback()"
            }
            .into();
            let args = if cdata.is_empty() {
                Vec::new()
            } else {
                vec![cdata.to_string()]
            };
            DecodedCallTrace {
                label,
                call_data: Some(DecodedCallData { signature, args }),
                return_data: if !trace.success {
                    Some(
                        self.revert_decoder
                            .decode(&trace.output, Some(trace.status)),
                    )
                } else {
                    None
                },
            }
        }
    }

    /// Decodes a function's input into the given trace.
    fn decode_function_input(&self, trace: &CallTrace, func: &Function) -> DecodedCallData {
        let mut args = None;
        if trace.data.len() >= edr_defaults::SELECTOR_LEN {
            if trace.address == CHEATCODE_ADDRESS {
                // Try to decode cheatcode inputs in a more custom way
                if let Some(v) = self.decode_cheatcode_inputs(func, &trace.data) {
                    args = Some(v);
                }
            }

            if args.is_none() {
                if let Ok(v) = func.abi_decode_input(&trace.data[edr_defaults::SELECTOR_LEN..]) {
                    args = Some(v.iter().map(|value| self.apply_label(value)).collect());
                }
            }
        }

        DecodedCallData {
            signature: func.signature(),
            args: args.unwrap_or_default(),
        }
    }

    /// Custom decoding for cheatcode inputs.
    fn decode_cheatcode_inputs(&self, func: &Function, data: &[u8]) -> Option<Vec<String>> {
        match func.name.as_str() {
            "expectRevert" => Some(vec![self.revert_decoder.decode(data, None)]),
            "addr" | "createWallet" | "deriveKey" | "rememberKey" => {
                // Redact private key in all cases
                Some(vec!["<pk>".to_string()])
            }
            "broadcast" | "startBroadcast" => {
                // Redact private key if defined
                // broadcast(uint256) / startBroadcast(uint256)
                if !func.inputs.is_empty() && func.inputs[0].ty == "uint256" {
                    Some(vec!["<pk>".to_string()])
                } else {
                    None
                }
            }
            "getNonce" => {
                // Redact private key if defined
                // getNonce(Wallet)
                if !func.inputs.is_empty() && func.inputs[0].ty == "tuple" {
                    Some(vec!["<pk>".to_string()])
                } else {
                    None
                }
            }
            "sign" | "signP256" => {
                let mut decoded = func.abi_decode_input(&data[edr_defaults::SELECTOR_LEN..]).ok()?;

                // Redact private key and replace in trace
                // sign(uint256,bytes32) / signP256(uint256,bytes32) / sign(Wallet,bytes32)
                if !decoded.is_empty() &&
                    (func.inputs[0].ty == "uint256" || func.inputs[0].ty == "tuple")
                {
                    decoded[0] = DynSolValue::String("<pk>".to_string());
                }

                Some(decoded.iter().map(format_token).collect())
            }
            "parseJson" |
            "parseJsonUint" |
            "parseJsonUintArray" |
            "parseJsonInt" |
            "parseJsonIntArray" |
            "parseJsonString" |
            "parseJsonStringArray" |
            "parseJsonAddress" |
            "parseJsonAddressArray" |
            "parseJsonBool" |
            "parseJsonBoolArray" |
            "parseJsonBytes" |
            "parseJsonBytesArray" |
            "parseJsonBytes32" |
            "parseJsonBytes32Array" |
            "writeJson" |
            // `keyExists` is being deprecated in favor of `keyExistsJson`. It will be removed in future versions.
            "keyExists" |
            "keyExistsJson" |
            "serializeBool" |
            "serializeUint" |
            "serializeUintToHex" |
            "serializeInt" |
            "serializeAddress" |
            "serializeBytes32" |
            "serializeString" |
            "serializeBytes" => {
                if self.verbose_cheatcode_decoding {
                    None
                } else {
                    let mut decoded = func.abi_decode_input(&data[edr_defaults::SELECTOR_LEN..]).ok()?;
                    let token = if func.name.as_str() == "parseJson" ||
                        // `keyExists` is being deprecated in favor of `keyExistsJson`. It will be removed in future versions.
                        func.name.as_str() == "keyExists" ||
                        func.name.as_str() == "keyExistsJson"
                    {
                        "<JSON file>"
                    } else {
                        "<stringified JSON>"
                    };
                    decoded[0] = DynSolValue::String(token.to_string());
                    Some(decoded.iter().map(format_token).collect())
                }
            }
            s if s.contains("Toml") => {
                if self.verbose_cheatcode_decoding {
                    None
                } else {
                    let mut decoded = func.abi_decode_input(&data[edr_defaults::SELECTOR_LEN..]).ok()?;
                    let token = if func.name.as_str() == "parseToml" ||
                        func.name.as_str() == "keyExistsToml"
                    {
                        "<TOML file>"
                    } else {
                        "<stringified TOML>"
                    };
                    decoded[0] = DynSolValue::String(token.to_string());
                    Some(decoded.iter().map(format_token).collect())
                }
            }
            _ => None,
        }
    }

    /// Decodes a function's output into the given trace.
    fn decode_function_output(&self, trace: &CallTrace, funcs: &[Function]) -> Option<String> {
        let data = &trace.output;
        if trace.success {
            if trace.address == CHEATCODE_ADDRESS {
                if let Some(decoded) = funcs
                    .iter()
                    .find_map(|func| self.decode_cheatcode_outputs(func))
                {
                    return Some(decoded);
                }
            }

            if let Some(values) = funcs
                .iter()
                .find_map(|func| func.abi_decode_output(data).ok())
            {
                // Functions coming from an external database do not have any outputs specified,
                // and will lead to returning an empty list of values.
                if values.is_empty() {
                    return None;
                }

                return Some(
                    values
                        .iter()
                        .map(|value| self.apply_label(value))
                        .format(", ")
                        .to_string(),
                );
            }

            None
        } else {
            Some(self.revert_decoder.decode(data, Some(trace.status)))
        }
    }

    /// Custom decoding for cheatcode outputs.
    fn decode_cheatcode_outputs(&self, func: &Function) -> Option<String> {
        match func.name.as_str() {
            s if s.starts_with("env") => Some("<env var value>"),
            "createWallet" | "deriveKey" => Some("<pk>"),
            "promptSecret" => Some("<secret>"),
            "parseJson" if !self.verbose_cheatcode_decoding => Some("<encoded JSON value>"),
            "readFile" if !self.verbose_cheatcode_decoding => Some("<file>"),
            _ => None,
        }
        .map(Into::into)
    }

    /// Decodes an event.
    pub async fn decode_event(&self, log: &LogData) -> DecodedCallLog {
        let &[t0, ..] = log.topics() else {
            return DecodedCallLog {
                name: None,
                params: None,
            };
        };

        let mut events = Vec::new();
        // The Clippy suggestion makes the code more difficult to read in this case.
        #[allow(clippy::single_match_else)]
        let events = match self.events.get(&(t0, log.topics().len() - 1)) {
            Some(es) => es,
            None => {
                if let Some(identifier) = &self.signature_identifier {
                    if let Some(event) = identifier.write().await.identify_event(&t0[..]).await {
                        events.push(get_indexed_event(event, log));
                    }
                }
                &events
            }
        };
        for event in events {
            if let Ok(decoded) = event.decode_log(log) {
                let params = reconstruct_params(event, &decoded);
                return DecodedCallLog {
                    name: Some(event.name.clone()),
                    params: Some(
                        params
                            .into_iter()
                            .zip(event.inputs.iter())
                            .map(|(param, input)| {
                                // undo patched names
                                let name = input.name.clone();
                                (name, self.apply_label(&param))
                            })
                            .collect(),
                    ),
                };
            }
        }

        DecodedCallLog {
            name: None,
            params: None,
        }
    }

    /// Prefetches function and event signatures into the identifier cache
    pub async fn prefetch_signatures(&self, nodes: &[CallTraceNode]) {
        const DEFAULT_CREATE2_DEPLOYER_BYTES: [u8; 20] = DEFAULT_CREATE2_DEPLOYER.0 .0;

        let Some(identifier) = &self.signature_identifier else {
            return;
        };

        let events_it = nodes
            .iter()
            .flat_map(|node| {
                node.logs
                    .iter()
                    .filter_map(|log| log.raw_log.topics().first())
            })
            .unique();
        identifier.write().await.identify_events(events_it).await;

        let funcs_it = nodes
            .iter()
            .filter_map(|n| match n.trace.address.0 .0 {
                DEFAULT_CREATE2_DEPLOYER_BYTES
                | [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0x01..=0x0a] => None,
                _ => n.trace.data.get(..edr_defaults::SELECTOR_LEN),
            })
            .filter(|v| !self.functions.contains_key(*v))
            .unique();
        identifier.write().await.identify_functions(funcs_it).await;
    }

    fn apply_label(&self, value: &DynSolValue) -> String {
        if let DynSolValue::Address(addr) = value {
            if let Some(label) = self.labels.get(addr) {
                return format!("{label}: [{addr}]");
            }
        }
        format_token(value)
    }
}

/// Restore the order of the params of a decoded event,
/// as Alloy returns the indexed and unindexed params separately.
fn reconstruct_params(event: &Event, decoded: &DecodedEvent) -> Vec<DynSolValue> {
    let mut indexed = 0;
    let mut unindexed = 0;
    let mut inputs = vec![];
    for input in event.inputs.iter() {
        if input.indexed {
            inputs.push(decoded.indexed[indexed].clone());
            indexed += 1;
        } else {
            inputs.push(decoded.body[unindexed].clone());
            unindexed += 1;
        }
    }

    inputs
}

fn indexed_inputs(event: &Event) -> usize {
    event.inputs.iter().filter(|param| param.indexed).count()
}

#[cfg(test)]
mod tests {
    use alloy_primitives::hex;

    use super::*;

    #[test]
    fn test_should_redact_pk() {
        let decoder = CallTraceDecoder::new();

        // [function_signature, data, expected]
        let cheatcode_input_test_cases = vec![
            // Should redact private key from traces in all cases:
            ("addr(uint256)", vec![], Some(vec!["<pk>".to_string()])),
            (
                "createWallet(string)",
                vec![],
                Some(vec!["<pk>".to_string()]),
            ),
            (
                "createWallet(uint256)",
                vec![],
                Some(vec!["<pk>".to_string()]),
            ),
            (
                "deriveKey(string,uint32)",
                vec![],
                Some(vec!["<pk>".to_string()]),
            ),
            (
                "deriveKey(string,string,uint32)",
                vec![],
                Some(vec!["<pk>".to_string()]),
            ),
            (
                "deriveKey(string,uint32,string)",
                vec![],
                Some(vec!["<pk>".to_string()]),
            ),
            (
                "deriveKey(string,string,uint32,string)",
                vec![],
                Some(vec!["<pk>".to_string()]),
            ),
            (
                "rememberKey(uint256)",
                vec![],
                Some(vec!["<pk>".to_string()]),
            ),
            //
            // Should redact private key from traces in specific cases with exceptions:
            ("broadcast(uint256)", vec![], Some(vec!["<pk>".to_string()])),
            ("broadcast()", vec![], None), // Ignore: `private key` is not passed.
            (
                "startBroadcast(uint256)",
                vec![],
                Some(vec!["<pk>".to_string()]),
            ),
            ("startBroadcast()", vec![], None), // Ignore: `private key` is not passed.
            (
                "getNonce((address,uint256,uint256,uint256))",
                vec![],
                Some(vec!["<pk>".to_string()]),
            ),
            ("getNonce(address)", vec![], None), // Ignore: `address` is public.
            //
            // Should redact private key and replace in trace in cases:
            (
                "sign(uint256,bytes32)",
                hex!(
                    "
                    e341eaa4
                    7c852118294e51e653712a81e05800f419141751be58f605c371e15141b007a6
                    0000000000000000000000000000000000000000000000000000000000000000
                "
                )
                .to_vec(),
                Some(vec![
                    "\"<pk>\"".to_string(),
                    "0x0000000000000000000000000000000000000000000000000000000000000000"
                        .to_string(),
                ]),
            ),
            (
                "signP256(uint256,bytes32)",
                hex!(
                    "
                    83211b40
                    7c852118294e51e653712a81e05800f419141751be58f605c371e15141b007a6
                    0000000000000000000000000000000000000000000000000000000000000000
                "
                )
                .to_vec(),
                Some(vec![
                    "\"<pk>\"".to_string(),
                    "0x0000000000000000000000000000000000000000000000000000000000000000"
                        .to_string(),
                ]),
            ),
        ];

        // [function_signature, expected]
        let cheatcode_output_test_cases = vec![
            // Should redact private key on output in all cases:
            ("createWallet(string)", Some("<pk>".to_string())),
            ("deriveKey(string,uint32)", Some("<pk>".to_string())),
        ];

        for (function_signature, data, expected) in cheatcode_input_test_cases {
            let function = Function::parse(function_signature).unwrap();
            let result = decoder.decode_cheatcode_inputs(&function, &data);
            assert_eq!(
                result, expected,
                "Input case failed for: {function_signature}"
            );
        }

        for (function_signature, expected) in cheatcode_output_test_cases {
            let function = Function::parse(function_signature).unwrap();
            let result = Some(
                decoder
                    .decode_cheatcode_outputs(&function)
                    .unwrap_or_default(),
            );
            assert_eq!(
                result, expected,
                "Output case failed for: {function_signature}"
            );
        }
    }
}
