use core::fmt::Debug;
use std::collections::HashMap;

use edr_eth::{BlockSpec, B256};
use edr_evm::{state::StateOverrides, trace::Trace, DebugTraceResult, DebugTraceResultWithTraces};
use edr_rpc_eth::CallRequest;
use serde::{Deserialize, Deserializer};

use crate::{
    data::ProviderData,
    requests::{
        eth::{resolve_block_spec_for_call_request, resolve_call_request},
        validation::validate_call_request,
    },
    time::TimeSinceEpoch,
    ProviderError,
};

pub fn handle_debug_trace_transaction<LoggerErrorT: Debug, TimerT: Clone + TimeSinceEpoch>(
    data: &mut ProviderData<LoggerErrorT, TimerT>,
    transaction_hash: B256,
    config: Option<DebugTraceConfig>,
) -> Result<(RpcDebugTraceResult, Vec<Trace>), ProviderError<LoggerErrorT>> {
    let DebugTraceResultWithTraces { result, traces } = data
        .debug_trace_transaction(
            &transaction_hash,
            config.map(Into::into).unwrap_or_default(),
        )
        .map_err(|error| match error {
            ProviderError::InvalidTransactionHash(tx_hash) => ProviderError::InvalidInput(format!(
                "Unable to find a block containing transaction {tx_hash}"
            )),
            _ => error,
        })?;

    let result = normalise_rpc_debug_trace(result);

    Ok((result, traces))
}

pub fn handle_debug_trace_call<LoggerErrorT: Debug, TimerT: Clone + TimeSinceEpoch>(
    data: &mut ProviderData<LoggerErrorT, TimerT>,
    call_request: CallRequest,
    block_spec: Option<BlockSpec>,
    config: Option<DebugTraceConfig>,
) -> Result<(RpcDebugTraceResult, Vec<Trace>), ProviderError<LoggerErrorT>> {
    let block_spec = resolve_block_spec_for_call_request(block_spec);
    validate_call_request(data.spec_id(), &call_request, &block_spec)?;

    let transaction =
        resolve_call_request(data, call_request, &block_spec, &StateOverrides::default())?;

    let DebugTraceResultWithTraces { result, traces } = data.debug_trace_call(
        transaction,
        &block_spec,
        config.map(Into::into).unwrap_or_default(),
    )?;

    let result = normalise_rpc_debug_trace(result);

    Ok((result, traces))
}

/// Config options for `debug_traceTransaction`
#[derive(Clone, Debug, Default, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DebugTraceConfig {
    /// Which tracer to use. This argument is currently unsupported.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(deserialize_with = "deserialize_tracer")]
    #[serde(default)]
    pub tracer: Option<Tracer>,
    /// Disable storage trace.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(default)]
    pub disable_storage: Option<bool>,
    /// Disable memory trace.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(default)]
    pub disable_memory: Option<bool>,
    /// Disable stack trace.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(default)]
    pub disable_stack: Option<bool>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
pub enum Tracer {
    #[default]
    #[serde(rename = "default")]
    Default,
}

fn deserialize_tracer<'de, DeserializerT>(
    deserializer: DeserializerT,
) -> Result<Option<Tracer>, DeserializerT::Error>
where
    DeserializerT: Deserializer<'de>,
{
    const HARDHAT_ERROR: &str = "Hardhat currently only supports the default tracer, so no tracer parameter should be passed.";

    let tracer = Option::<Tracer>::deserialize(deserializer)
        .map_err(|_error| serde::de::Error::custom(HARDHAT_ERROR))?;

    if tracer.is_some() {
        Err(serde::de::Error::custom(HARDHAT_ERROR))
    } else {
        Ok(tracer)
    }
}

impl From<DebugTraceConfig> for edr_evm::DebugTraceConfig {
    fn from(value: DebugTraceConfig) -> Self {
        let DebugTraceConfig {
            disable_storage,
            disable_memory,
            disable_stack,
            // Tracer argument is not supported by Hardhat
            tracer: _,
        } = value;
        Self {
            disable_storage: disable_storage.unwrap_or_default(),
            disable_memory: disable_memory.unwrap_or_default(),
            disable_stack: disable_stack.unwrap_or_default(),
        }
    }
}

/// This is the JSON-RPC Debug trace format
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RpcDebugTraceResult {
    pub failed: bool,
    pub gas: u64,
    // Adding pass and gass used since Hardhat tests still
    // depend on them
    pub pass: bool,
    pub gas_used: u64,
    pub return_value: String,
    pub struct_logs: Vec<RpcDebugTraceLogItem>,
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RpcDebugTraceLogItem {
    /// Program Counter
    pub pc: u64,
    /// Name of the operation
    pub op: String,
    /// Name of the operation (Needed for Hardhat tests)
    pub op_name: String,
    /// Gas left before executing this operation as hex number.
    pub gas: u64,
    /// Gas cost of this operation as hex number.
    pub gas_cost: u64,
    /// Array of all values (hex numbers) on the stack
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stack: Option<Vec<String>>,
    /// Depth of the call stack
    pub depth: u64,
    /// Size of memory array
    pub mem_size: u64,
    /// Description of an error as a hex string.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    /// Array of all allocated values as hex strings.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub memory: Option<Vec<String>>,
    /// Map of all stored values with keys and values encoded as hex strings.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub storage: Option<HashMap<String, String>>,
}

// Rust port of https://github.com/NomicFoundation/hardhat/blob/024d72b09c6edefb00c012e9514a0948c255d0ab/v-next/hardhat/src/internal/builtin-plugins/network-manager/edr/utils/convert-to-edr.ts#L176
/// This normalization is done because this is the format Hardhat expects
fn normalise_rpc_debug_trace(trace: DebugTraceResult) -> RpcDebugTraceResult {
    let mut struct_logs = Vec::new();

    for log in trace.logs {
        let rpc_log = RpcDebugTraceLogItem {
            pc: log.pc,
            op: log.op_name.clone(),
            op_name: log.op_name,
            gas: u64::from_str_radix(log.gas.trim_start_matches("0x"), 16).unwrap_or(0),
            gas_cost: u64::from_str_radix(log.gas_cost.trim_start_matches("0x"), 16).unwrap_or(0),
            stack: log.stack.map(|values| {
                values
                    .into_iter()
                    // Removing this trim temporarily as the Hardhat test assumes 0x is there
                    // .map(|value| value.trim_start_matches("0x").to_string())
                    .collect()
            }),
            depth: log.depth,
            mem_size: log.mem_size,
            error: log.error,
            memory: log.memory,
            storage: log.storage.map(|storage| {
                storage
                    .into_iter()
                    // Removing this trim temporarily as the Hardhat test assumes 0x is there
                    // .map(|(key, value)| {
                    //     let stripped_key = key.strip_prefix("0x").unwrap_or(&key).to_string();
                    //     let stripped_value =
                    // value.strip_prefix("0x").unwrap_or(&value).to_string();
                    //     (stripped_key, stripped_value)
                    // })
                    .collect()
            }),
        };

        struct_logs.push(rpc_log);
    }

    // REVM trace adds initial STOP that Hardhat doesn't expect
    if !struct_logs.is_empty() && struct_logs[0].op == "STOP" {
        struct_logs.remove(0);
    }

    let return_value = trace
        .output
        .map(|b| b.to_string().trim_start_matches("0x").to_string())
        .unwrap_or_default();

    RpcDebugTraceResult {
        failed: !trace.pass,
        gas: trace.gas_used,
        pass: trace.pass,
        gas_used: trace.gas_used,
        return_value,
        struct_logs,
    }
}
