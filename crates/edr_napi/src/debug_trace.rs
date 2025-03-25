use std::collections::HashMap;

use napi::bindgen_prelude::{BigInt, Buffer};
use napi_derive::napi;

#[napi(object)]
pub struct DebugTraceResult {
    pub pass: bool,
    pub gas_used: BigInt,
    pub output: Option<Buffer>,
    pub struct_logs: Vec<DebugTraceLogItem>,
}

#[napi(object)]
pub struct DebugTraceLogItem {
    /// Program Counter
    pub pc: BigInt,
    // Op code
    pub op: u8,
    /// Gas left before executing this operation as hex number.
    pub gas: String,
    /// Gas cost of this operation as hex number.
    pub gas_cost: String,
    /// Array of all values (hex numbers) on the stack
    pub stack: Option<Vec<String>>,
    /// Depth of the call stack
    pub depth: BigInt,
    /// Size of memory array
    pub mem_size: BigInt,
    /// Name of the operation
    pub op_name: String,
    /// Description of an error as a hex string.
    pub error: Option<String>,
    /// Array of all allocated values as hex strings.
    pub memory: Option<Vec<String>>,
    /// Map of all stored values with keys and values encoded as hex strings.
    pub storage: Option<HashMap<String, String>>,
}

/// Result of `debug_traceTransaction` and `debug_traceCall` after
/// normalisation. `pass` and `gas_used` exist before normalisation. They will
/// be replaced by `failed` and `gas` respectively. They currently exist
/// together because Hardhat still depends on them but `pass` and `gas_used`
/// should likely to be removed after a while.
#[napi(object)]
pub struct RpcDebugTraceResult {
    /// Whether transaction was executed successfully.
    pub failed: bool,
    /// All gas used by the transaction.
    /// This field is similar to gas_used but it is what Hardhat expects after
    /// normalisation
    pub gas: BigInt,
    /// Whether transaction was executed successfully.
    /// This field is similar to failed but it is what Hardhat expects after
    /// normalisation
    pub pass: bool,
    /// All gas used by the transaction.
    pub gas_used: BigInt,
    /// Return values of the function.
    pub return_value: String,
    /// Debug logs after normalisation
    pub struct_logs: Vec<RpcDebugTraceLogItem>,
}

/// Debug logs after normalising the EIP-3155 debug logs.
/// This is the format Hardhat expects
#[napi(object)]
pub struct RpcDebugTraceLogItem {
    /// Program Counter
    pub pc: BigInt,
    /// Name of the operation
    pub op: String,
    /// Gas left before executing this operation as hex number.
    pub gas: BigInt,
    /// Gas cost of this operation as hex number.
    pub gas_cost: BigInt,
    /// Array of all values (hex numbers) on the stack
    pub stack: Option<Vec<String>>,
    /// Depth of the call stack
    pub depth: BigInt,
    /// Size of memory array
    pub mem_size: BigInt,
    /// Description of an error as a hex string.
    pub error: Option<String>,
    /// Array of all allocated values as hex strings.
    pub memory: Option<Vec<String>>,
    /// Map of all stored values with keys and values encoded as hex strings.
    pub storage: Option<HashMap<String, String>>,
}
