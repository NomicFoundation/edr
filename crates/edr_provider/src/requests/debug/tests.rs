//! Pins the JSON-RPC contract of `debug_traceTransaction` and
//! `debug_traceCall`.
//!
//! The reference is the [Ethereum execution-apis spec] at the pinned spec
//! commit. Its opcode logger is defined in [opcode-tracer.yaml], and its
//! method clauses are in [trace.yaml]. A later spec change is adopted here
//! deliberately. The `spec` module holds tests per requirement of the opcode
//! logger and the method clauses. The `edr` module holds behaviour that the
//! spec leaves undefined.
//!
//! [Ethereum execution-apis spec]: https://ethereum.github.io/execution-apis/
//! [opcode-tracer.yaml]: https://github.com/ethereum/execution-apis/blob/3284ee80c3da3e516c53782dde75157cccf1ebdc/src/schemas/opcode-tracer.yaml
//! [trace.yaml]: https://github.com/ethereum/execution-apis/blob/3284ee80c3da3e516c53782dde75157cccf1ebdc/src/debug/trace.yaml

mod edr;
mod spec;

use alloy_rpc_types_trace::geth::GethDebugTracingOptions;
use edr_block_header::HeaderOverrides;
use edr_chain_l1::{rpc::call::L1CallRequest, L1ChainSpec, L1Hardfork};
use edr_eth::BlockSpec;
use edr_primitives::{
    address,
    bytecode::opcode::{self, OpCode},
    Address, Bytes, B256, U256,
};
use edr_rpc_eth::jsonrpc;
use edr_transaction::{request::TransactionRequestAndSender, TxKind};
use revm_inspectors::tracing::types::CallKind as TraceCallKind;
use serde_json::{json, Map, Value};

use super::{handle_debug_trace_call, handle_debug_trace_transaction};
use crate::{
    error::{INVALID_INPUT, INVALID_PARAMS},
    test_utils::ProviderTestFixture,
    DebugTraceError, ProviderError, ProviderErrorForChainSpec,
};

/// The contract under test, installed by the tests that need code.
const CONTRACT: Address = address!("0x1111111111111111111111111111111111111111");
/// The contract that `CONTRACT` calls.
const CALLEE: Address = address!("0x2222222222222222222222222222222222222222");
/// An account without code that receives value transfers.
const RECIPIENT: Address = address!("0x3333333333333333333333333333333333333333");
/// The ecrecover precompile.
const ECRECOVER: Address = address!("0x0000000000000000000000000000000000000001");

/// The hardfork of every fixture. The gas constants assume its schedule,
/// which Amsterdam changes.
const HARDFORK: L1Hardfork = L1Hardfork::Osaka;

/// Gas limit of transactions that must not run out of gas, in gas units.
const GAS_LIMIT: u64 = 1_000_000;
/// Intrinsic gas of a transaction without calldata or access list, in gas
/// units. It is the Yellow Paper's `G_transaction`, which Amsterdam lowers
/// under EIP-2780.
const TRANSACTION_BASE_GAS: u64 = 21_000;
/// Execution gas that the infinite loop exhausts, in gas units.
const LOOP_GAS_BUDGET: u64 = 9_000;
/// Gas limit of the infinite loop transaction, in gas units.
const LOOP_GAS_LIMIT: u64 = TRANSACTION_BASE_GAS + LOOP_GAS_BUDGET;
/// Gas charge of the ecrecover precompile, in gas units.
const ECRECOVER_GAS: u64 = 3_000;
/// Execution gas left to a precompile call, in gas units.
const PRECOMPILE_GAS_BUDGET: u64 = 1_000;
const _: () = assert!(
    PRECOMPILE_GAS_BUDGET < ECRECOVER_GAS,
    "the precompile call must run out of gas"
);
/// Gas limit of the precompile call transaction, in gas units.
const PRECOMPILE_GAS_LIMIT: u64 = TRANSACTION_BASE_GAS + PRECOMPILE_GAS_BUDGET;
/// Value of the value transfer, in wei.
const TRANSFER_VALUE: u64 = 1;

/// Gas cost of `PUSH1`, in gas units. It is the Yellow Paper's `G_verylow`.
const VERY_LOW_GAS: u64 = 3;
/// Gas cost of a cold storage slot access, in gas units (EIP-2929).
const COLD_SLOAD_COST: u64 = 2_100;
/// Gas cost of a warm storage slot read, in gas units (EIP-2929).
const WARM_STORAGE_READ_GAS: u64 = 100;
/// Gas cost of setting a zero storage slot to non-zero, in gas units
/// (EIP-2200).
const SSTORE_SET_GAS: u64 = 20_000;
/// Refund counter after a slot set from zero is restored to zero in the same
/// transaction, in gas units (EIP-2200, as amended by EIP-2929).
const RESTORE_REFUND: u64 = SSTORE_SET_GAS - WARM_STORAGE_READ_GAS;
/// The divisor of gas used that caps the applied refund (EIP-3529).
const MAX_REFUND_QUOTIENT: u64 = 5;
/// The number of `PUSH1` steps in `WRITE_THEN_RESTORE_CODE`.
const WRITE_THEN_RESTORE_PUSH_COUNT: u64 = 4;
/// Gas used by the `WRITE_THEN_RESTORE_CODE` transaction before its refund,
/// in gas units. The first `SSTORE` sets a cold slot, so EIP-2929 adds
/// `COLD_SLOAD_COST` on top of `SSTORE_SET_GAS`. The second `SSTORE` is warm.
const WRITE_THEN_RESTORE_GAS_BEFORE_REFUND: u64 = TRANSACTION_BASE_GAS
    + WRITE_THEN_RESTORE_PUSH_COUNT * VERY_LOW_GAS
    + COLD_SLOAD_COST
    + SSTORE_SET_GAS
    + WARM_STORAGE_READ_GAS;
/// The largest refund that EIP-3529 applies to the `WRITE_THEN_RESTORE_CODE`
/// transaction, in gas units.
const WRITE_THEN_RESTORE_REFUND_CAP: u64 =
    WRITE_THEN_RESTORE_GAS_BEFORE_REFUND / MAX_REFUND_QUOTIENT;
const _: () = assert!(
    RESTORE_REFUND > WRITE_THEN_RESTORE_REFUND_CAP,
    "the cap must apply to the restore refund"
);
/// Gas used by the `WRITE_THEN_RESTORE_CODE` transaction after its refund, in
/// gas units. `RESTORE_REFUND` exceeds the cap, so the cap is refunded.
const WRITE_THEN_RESTORE_GAS_USED: u64 =
    WRITE_THEN_RESTORE_GAS_BEFORE_REFUND - WRITE_THEN_RESTORE_REFUND_CAP;
/// The `limit` option, in steps. It is non-zero and below the step count of
/// `READ_THEN_WRITE_CODE`.
const STEP_LIMIT: usize = 3;

/// The storage slot that the storage contracts read and write.
const SLOT: u8 = 0;
/// The storage slot that `CONTRACT` writes before it calls `CALLEE`.
const CALLER_SLOT: u8 = 1;
/// The storage slot that `WRITER_CODE` writes.
const WRITER_SLOT: u8 = 2;
/// The value that contracts store, return and revert with.
const STORED_VALUE: u8 = 0x42;
/// The value of a storage slot that was never written.
const UNSET_VALUE: u8 = 0;
/// The refund counter before any refund is earned, in gas units.
const NO_REFUND: u64 = 0;
/// The size of an EVM word, in bytes.
const WORD_SIZE: u8 = 32;
/// An opcode byte that no hardfork defines.
const UNDEFINED_OPCODE: u8 = 0x0c;
/// The number of offset and size arguments of a call's input and output
/// ranges.
const CALL_RANGE_ARGUMENT_COUNT: usize = 4;
/// The code offset of the `JUMPDEST` that `INFINITE_LOOP_CODE` jumps to.
const LOOP_START: u8 = 0;

/// The `depth` of the transaction's own frame.
const TOP_LEVEL_DEPTH: u64 = 1;
/// The `depth` of a frame that the top-level frame calls.
const CALLEE_DEPTH: u64 = TOP_LEVEL_DEPTH + 1;

/// The `op` name of an undefined opcode. REVM has no name for it, so the
/// string is revm-inspectors' choice.
const UNKNOWN_OP: &str = "Unknown";
/// The step error of a frame that ran out of gas, as revm-inspectors formats
/// the optional halt status with `Debug`.
const HALT_OUT_OF_GAS: &str = "Some(OutOfGas)";
/// A tracer name that is not built in, which alloy parses as a JavaScript
/// tracer.
const UNKNOWN_TRACER: &str = "unknownTracer";

/// Reads `SLOT`, then writes `STORED_VALUE` to it.
const READ_THEN_WRITE_CODE: [u8; 10] = [
    opcode::PUSH1,
    SLOT,
    opcode::SLOAD,
    opcode::POP,
    opcode::PUSH1,
    STORED_VALUE,
    opcode::PUSH1,
    SLOT,
    opcode::SSTORE,
    opcode::STOP,
];
/// Writes `STORED_VALUE` to `SLOT`, then restores its original zero.
const WRITE_THEN_RESTORE_CODE: [u8; 11] = [
    opcode::PUSH1,
    STORED_VALUE,
    opcode::PUSH1,
    SLOT,
    opcode::SSTORE,
    opcode::PUSH1,
    UNSET_VALUE,
    opcode::PUSH1,
    SLOT,
    opcode::SSTORE,
    opcode::STOP,
];
/// Reads `SLOT`, then writes `STORED_VALUE` to `CALLER_SLOT`.
const READ_THEN_WRITE_OTHER_SLOT_CODE: [u8; 10] = [
    opcode::PUSH1,
    SLOT,
    opcode::SLOAD,
    opcode::POP,
    opcode::PUSH1,
    STORED_VALUE,
    opcode::PUSH1,
    CALLER_SLOT,
    opcode::SSTORE,
    opcode::STOP,
];
/// Reads `SLOT` twice, so the second read is warm.
const READ_TWICE_CODE: [u8; 9] = [
    opcode::PUSH1,
    SLOT,
    opcode::SLOAD,
    opcode::POP,
    opcode::PUSH1,
    SLOT,
    opcode::SLOAD,
    opcode::POP,
    opcode::STOP,
];
/// Writes `STORED_VALUE` to `WRITER_SLOT`.
const WRITER_CODE: [u8; 6] = [
    opcode::PUSH1,
    STORED_VALUE,
    opcode::PUSH1,
    WRITER_SLOT,
    opcode::SSTORE,
    opcode::STOP,
];
/// Jumps back to its start until it runs out of gas.
const INFINITE_LOOP_CODE: [u8; 4] = [opcode::JUMPDEST, opcode::PUSH1, LOOP_START, opcode::JUMP];
/// Deploys a contract without code.
const EMPTY_INIT_CODE: [u8; 3] = [opcode::PUSH0, opcode::PUSH0, opcode::RETURN];

/// Stores `STORED_VALUE` as the first memory word, then halts with `halt`
/// and that word as output.
fn return_word_code(halt: u8) -> Vec<u8> {
    vec![
        opcode::PUSH1,
        STORED_VALUE,
        opcode::PUSH0,
        opcode::MSTORE,
        opcode::PUSH1,
        WORD_SIZE,
        opcode::PUSH0,
        halt,
    ]
}

/// The opcode that `caller_code` calls `CALLEE` through.
#[derive(Clone, Copy)]
enum CallKind {
    Call,
    DelegateCall,
}

impl CallKind {
    /// Returns the call opcode.
    fn opcode(self) -> u8 {
        match self {
            CallKind::Call => opcode::CALL,
            CallKind::DelegateCall => opcode::DELEGATECALL,
        }
    }

    /// Returns whether the opcode takes a value argument.
    fn takes_value(self) -> bool {
        match self {
            CallKind::Call => true,
            CallKind::DelegateCall => false,
        }
    }
}

/// Writes `STORED_VALUE` to `CALLER_SLOT` and calls `CALLEE` through `kind`
/// with empty ranges and no value. Then reads `CALLER_SLOT`.
fn caller_code(kind: CallKind) -> Vec<u8> {
    let mut code = vec![
        opcode::PUSH1,
        STORED_VALUE,
        opcode::PUSH1,
        CALLER_SLOT,
        opcode::SSTORE,
    ];
    let zero_argument_count = CALL_RANGE_ARGUMENT_COUNT + usize::from(kind.takes_value());
    code.extend(std::iter::repeat_n(opcode::PUSH0, zero_argument_count));
    code.push(opcode::PUSH20);
    code.extend_from_slice(CALLEE.as_slice());
    code.extend_from_slice(&[
        opcode::GAS,
        kind.opcode(),
        opcode::POP,
        opcode::PUSH1,
        CALLER_SLOT,
        opcode::SLOAD,
        opcode::STOP,
    ]);
    code
}

/// Returns the byte offset and REVM opcode of every opcode in straight-line
/// `code`.
fn decode(code: &[u8]) -> Vec<(u64, OpCode)> {
    let mut decoded = Vec::new();
    let mut offset = 0;
    while let Some(&byte) = code.get(offset) {
        let opcode = OpCode::new(byte).expect("the code holds defined opcodes");
        decoded.push((
            u64::try_from(offset).expect("the offset fits in u64"),
            opcode,
        ));
        offset += 1 + usize::from(opcode.info().immediate_size());
    }
    decoded
}

/// Returns the byte offset of every opcode in straight-line `code`.
fn opcode_offsets(code: &[u8]) -> Vec<u64> {
    decode(code).into_iter().map(|(offset, _)| offset).collect()
}

/// Returns REVM's name of every opcode in straight-line `code`.
fn op_names(code: &[u8]) -> Vec<&'static str> {
    decode(code)
        .into_iter()
        .map(|(_, opcode)| opcode.as_str())
        .collect()
}

/// Pushes and pops the block number, then runs `code`.
fn number_then(code: &[u8]) -> Vec<u8> {
    let mut prefixed = vec![opcode::NUMBER, opcode::POP];
    prefixed.extend_from_slice(code);
    prefixed
}

/// Returns `step`'s `key` field as an integer.
fn integer_field(step: &Value, key: &str) -> u64 {
    field(step, key)
        .as_u64()
        .unwrap_or_else(|| panic!("`{key}` should be an integer in {step}"))
}

/// Returns `value` as the hex string of a 32-byte word.
fn word(value: u8) -> String {
    B256::with_last_byte(value).to_string()
}

/// Returns the JSON storage map of `(slot, value)` pairs.
fn storage(entries: &[(u8, u8)]) -> Value {
    let map: Map<String, Value> = entries
        .iter()
        .map(|&(slot, value)| (word(slot), Value::String(word(value))))
        .collect();
    Value::Object(map)
}

/// Parses tracing options from their JSON-RPC form.
fn tracing_options(options: Value) -> GethDebugTracingOptions {
    serde_json::from_value(options).expect("the tracing options are valid")
}

/// Returns the `key` field of `value`, and panics when it is absent.
fn field<'value>(value: &'value Value, key: &str) -> &'value Value {
    value
        .get(key)
        .unwrap_or_else(|| panic!("`{key}` should be present in {value}"))
}

/// Returns the struct logs of a default-tracer trace.
fn struct_logs(trace: &Value) -> &[Value] {
    field(trace, "structLogs")
        .as_array()
        .expect("`structLogs` is an array")
}

/// Returns the opcode name of a step.
fn op(step: &Value) -> &str {
    field(step, "op").as_str().expect("`op` is a string")
}

/// Returns the opcode names of all steps.
fn ops(trace: &Value) -> Vec<&str> {
    struct_logs(trace).iter().map(op).collect()
}

/// Returns the `depth` of every step whose opcode is `name`.
fn depths_of_op<'trace>(trace: &'trace Value, name: &str) -> Vec<&'trace Value> {
    struct_logs(trace)
        .iter()
        .filter(|step| op(step) == name)
        .map(|step| field(step, "depth"))
        .collect()
}

/// Returns the `index`th step.
fn nth_step(trace: &Value, index: usize) -> &Value {
    struct_logs(trace)
        .get(index)
        .unwrap_or_else(|| panic!("step {index} should exist in {trace}"))
}

/// Returns the index of the first step whose opcode is `name`.
fn op_index(trace: &Value, name: &str) -> usize {
    struct_logs(trace)
        .iter()
        .position(|step| op(step) == name)
        .unwrap_or_else(|| panic!("a `{name}` step should exist in {trace}"))
}

/// Returns the first step whose opcode is `name`.
fn step_with_op<'trace>(trace: &'trace Value, name: &str) -> &'trace Value {
    nth_step(trace, op_index(trace, name))
}

/// Returns the next step in the same frame as the first `name` step.
fn step_after_op<'trace>(trace: &'trace Value, name: &str) -> &'trace Value {
    let index = op_index(trace, name);
    let depth = field(nth_step(trace, index), "depth");
    struct_logs(trace)
        .iter()
        .skip(index + 1)
        .find(|next| field(next, "depth") == depth)
        .unwrap_or_else(|| panic!("the frame should continue after `{name}` in {trace}"))
}

/// Asserts that `trace` has more than one step.
fn assert_has_steps(trace: &Value) {
    assert!(struct_logs(trace).len() > 1, "too few steps in {trace}");
}

/// A local provider with a funded sender and installed contracts.
struct TraceFixture {
    provider: ProviderTestFixture<L1ChainSpec>,
    sender: Address,
}

impl TraceFixture {
    /// Creates a provider on `HARDFORK` with each `(address, code)` pair
    /// installed.
    fn new(contracts: &[(Address, &[u8])]) -> anyhow::Result<Self> {
        let mut provider = ProviderTestFixture::<L1ChainSpec>::new_local_with_hardfork(HARDFORK)?;
        let sender = provider.nth_local_account(0)?;
        for (address, code) in contracts {
            provider
                .provider_data
                .set_code(*address, Bytes::copy_from_slice(code))?;
        }

        Ok(Self { provider, sender })
    }

    /// Returns the number of the latest block.
    fn last_block_number(&self) -> u64 {
        self.provider.provider_data.last_block_number()
    }

    /// Enables or disables automining of sent transactions.
    fn set_auto_mining(&mut self, enabled: bool) {
        self.provider.provider_data.set_auto_mining(enabled);
    }

    /// Mines a block with the pending transactions, as `evm_mine` does.
    fn mine(&mut self) -> anyhow::Result<()> {
        self.provider
            .provider_data
            .mine_and_commit_block(HeaderOverrides::default())?;
        Ok(())
    }

    /// Signs and sends a transaction.
    fn send(
        &mut self,
        kind: TxKind,
        input: &[u8],
        gas_limit: u64,
        value: U256,
    ) -> anyhow::Result<B256> {
        let nonce = self
            .provider
            .provider_data
            .account_next_nonce(&self.sender)?;
        let request = self.provider.transaction_request(
            kind,
            Bytes::copy_from_slice(input),
            gas_limit,
            value,
            nonce,
        );
        let data = &mut self.provider.provider_data;
        let transaction = data.sign_transaction_request(TransactionRequestAndSender {
            request,
            sender: self.sender,
        })?;

        Ok(data.send_transaction(transaction)?.transaction_hash)
    }

    /// Returns the wire JSON of `debug_traceTransaction`.
    fn trace_transaction(
        &mut self,
        transaction_hash: B256,
        options: Option<Value>,
    ) -> Result<Value, ProviderErrorForChainSpec<L1ChainSpec>> {
        let (trace, _call_traces) = handle_debug_trace_transaction(
            &mut self.provider.provider_data,
            transaction_hash,
            options.map(tracing_options),
        )?;

        Ok(serde_json::to_value(&trace).expect("a trace serialises to JSON"))
    }

    /// Returns the wire JSON of `debug_traceCall`.
    fn trace_call(
        &mut self,
        to: Option<Address>,
        input: &[u8],
        block_spec: Option<BlockSpec>,
        options: Option<Value>,
    ) -> Result<Value, ProviderErrorForChainSpec<L1ChainSpec>> {
        let request = L1CallRequest {
            from: Some(self.sender),
            to,
            data: Some(Bytes::copy_from_slice(input)),
            ..L1CallRequest::default()
        };
        let (trace, _call_traces) = handle_debug_trace_call(
            &mut self.provider.provider_data,
            request,
            block_spec,
            options.map(tracing_options),
        )?;

        Ok(serde_json::to_value(&trace).expect("a trace serialises to JSON"))
    }
}

/// Installs `contracts`, sends a transaction to `CONTRACT` and traces it with
/// `options`.
fn trace_contract_transaction(
    contracts: &[(Address, &[u8])],
    options: Option<Value>,
) -> anyhow::Result<Value> {
    let mut fixture = TraceFixture::new(contracts)?;
    let transaction_hash = fixture.send(TxKind::Call(CONTRACT), &[], GAS_LIMIT, U256::ZERO)?;
    Ok(fixture.trace_transaction(transaction_hash, options)?)
}

/// Traces a transaction to `READ_THEN_WRITE_CODE`.
fn trace_read_then_write(options: Option<Value>) -> anyhow::Result<Value> {
    trace_contract_transaction(&[(CONTRACT, &READ_THEN_WRITE_CODE)], options)
}

/// Traces a transaction to `return_word_code` that returns.
fn trace_return_word(options: Option<Value>) -> anyhow::Result<Value> {
    trace_contract_transaction(&[(CONTRACT, &return_word_code(opcode::RETURN))], options)
}

/// Traces a transaction to `caller_code` whose `CALL` targets `callee_code`.
fn trace_transaction_calling(callee_code: &[u8], options: Option<Value>) -> anyhow::Result<Value> {
    trace_contract_transaction(
        &[
            (CONTRACT, &caller_code(CallKind::Call)),
            (CALLEE, callee_code),
        ],
        options,
    )
}

/// Asserts that `error` is the JS tracer rejection, with `INVALID_PARAMS` and
/// the message `JS Tracer is not enabled`. Pins the current rejection until
/// [#1250] enables JS tracers.
///
/// [#1250]: https://github.com/NomicFoundation/edr/issues/1250
fn assert_js_tracer_rejected(error: ProviderErrorForChainSpec<L1ChainSpec>) {
    assert!(
        matches!(
            error,
            ProviderError::DebugTrace(DebugTraceError::JsTracerNotEnabled)
        ),
        "unexpected error: {error:?}"
    );
    let wire_error = jsonrpc::Error::from(error);
    assert_eq!(wire_error.code, INVALID_PARAMS);
    assert_eq!(wire_error.message, "JS Tracer is not enabled");
}
