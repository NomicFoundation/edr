//! Behaviour that the execution-apis spec leaves undefined, pinned for EDR.
//!
//! - An undefined opcode is named `Unknown`, and a halting step's error is
//!   `Some(OutOfGas)`. The spec defines neither string. Both come from
//!   revm-inspectors, and EDR passes them through verbatim. A change to either
//!   string reaches every consumer of the trace, so it must be deliberate and
//!   get a changeset.
//! - `disableMemory` and `disableReturnData` are not spec options. alloy keeps
//!   them for Erigon compatibility, and `enableMemory` and `enableReturnData`
//!   take precedence over them. EDR passes the options through, so alloy's rule
//!   is EDR's rule. Users may send `disableMemory` and `disableReturnData`
//!   directly.
//! - The spec leaves the output of named tracers undefined. EDR returns geth's
//!   `callTracer` frame unchanged.
//! - The spec defines no error codes or messages for an unknown tracer or an
//!   unknown transaction hash. EDR's are part of its interface.
//! - The spec does not define `debug_traceCall`, so EDR follows geth. It
//!   defaults to the latest block, traces a creation without a recipient and
//!   persists no state.
//! - The spec's `StructLog` and `OpcodeTransactionTrace` schemas do not forbid
//!   additional properties. EDR pins their field sets anyway. EDR serialises
//!   alloy's `StructLog`, which revm-inspectors fills, verbatim. A field added
//!   by either crate would reach every consumer of the trace. Such a field must
//!   be deliberate and get a changeset. The test also asserts that the steps
//!   together carry every field, so an empty or partial trace cannot pass it.
//!   From Amsterdam, EDR adds the EIP-8037 `stateGasCost` and
//!   `stateGasReservoir` fields deliberately, and the `eip8037` integration
//!   tests pin them.

use std::collections::BTreeSet;

use super::*;

/// The properties of the spec's `OpcodeTransactionTrace` schema.
const TRACE_FIELDS: [&str; 4] = ["failed", "gas", "returnValue", "structLogs"];
/// The properties of the spec's `StructLog` schema.
const STRUCT_LOG_FIELDS: [&str; 11] = [
    "pc",
    "op",
    "gas",
    "gasCost",
    "depth",
    "error",
    "refund",
    "stack",
    "memory",
    "returnData",
    "storage",
];

/// Gas cost of a cold account access, in gas units (EIP-2929).
const COLD_ACCOUNT_ACCESS_COST: u64 = 2_600;
/// Gas limit of a transaction to `caller_code` whose callee loops, in gas
/// units. It pays the caller's cold `SSTORE` and cold `CALL`, and leaves
/// `LOOP_GAS_BUDGET` for the caller's other steps and the callee. The callee
/// exhausts all but the sixty-fourth that EIP-150 keeps back, and that share
/// pays the caller's steps after the call.
const CALLING_LOOP_GAS_LIMIT: u64 = TRANSACTION_BASE_GAS
    + COLD_SLOAD_COST
    + SSTORE_SET_GAS
    + COLD_ACCOUNT_ACCESS_COST
    + LOOP_GAS_BUDGET;

/// Returns the keys of a JSON object, and panics when `object` is not one.
fn keys(object: &Value) -> BTreeSet<&str> {
    object
        .as_object()
        .unwrap_or_else(|| panic!("an object is expected, got {object}"))
        .keys()
        .map(String::as_str)
        .collect()
}

#[test]
fn undefined_opcode_is_reported_as_unknown() -> anyhow::Result<()> {
    let trace = trace_contract_transaction(&[(CONTRACT, &[UNDEFINED_OPCODE])], None)?;

    assert_eq!(op(nth_step(&trace, 0)), UNKNOWN_OP);

    Ok(())
}

#[test]
fn halting_step_error_is_the_debug_formatted_halt_status() -> anyhow::Result<()> {
    let mut fixture = TraceFixture::new(&[(CONTRACT, &INFINITE_LOOP_CODE)])?;
    let transaction_hash = fixture.send(TxKind::Call(CONTRACT), &[], LOOP_GAS_LIMIT, U256::ZERO)?;

    let trace = fixture.trace_transaction(transaction_hash, None)?;

    let last_step = struct_logs(&trace).last().expect("the loop has steps");
    assert_eq!(field(last_step, "error"), &json!(HALT_OUT_OF_GAS));

    Ok(())
}

#[test]
fn disable_memory_false_enables_memory_when_enable_memory_is_absent() -> anyhow::Result<()> {
    let trace = trace_return_word(Some(json!({ "disableMemory": false })))?;

    let step = step_after_op(&trace, OpCode::MSTORE.as_str());

    assert_eq!(field(step, "memory"), &json!([word(STORED_VALUE)]));

    Ok(())
}

#[test]
fn disable_return_data_false_enables_return_data_when_enable_return_data_is_absent(
) -> anyhow::Result<()> {
    let trace = trace_transaction_calling(
        &return_word_code(opcode::RETURN),
        Some(json!({ "disableReturnData": false })),
    )?;

    assert_eq!(
        field(step_after_op(&trace, OpCode::CALL.as_str()), "returnData"),
        &json!(word(STORED_VALUE))
    );

    Ok(())
}

#[test]
fn enable_memory_overrides_disable_memory_to_capture_memory() -> anyhow::Result<()> {
    let trace = trace_return_word(Some(json!({
        "enableMemory": true,
        "disableMemory": true,
    })))?;

    let step = step_after_op(&trace, OpCode::MSTORE.as_str());

    assert_eq!(field(step, "memory"), &json!([word(STORED_VALUE)]));

    Ok(())
}

#[test]
fn enable_memory_overrides_disable_memory_to_omit_memory() -> anyhow::Result<()> {
    let trace = trace_return_word(Some(json!({
        "enableMemory": false,
        "disableMemory": false,
    })))?;

    let step = step_after_op(&trace, OpCode::MSTORE.as_str());

    assert_eq!(step.get("memory"), None);

    Ok(())
}

#[test]
fn call_tracer_returns_a_call_frame_with_nested_calls_and_no_struct_logs() -> anyhow::Result<()> {
    let mut fixture = TraceFixture::new(&[
        (CONTRACT, &caller_code(CallKind::Call)),
        (CALLEE, &WRITER_CODE),
    ])?;
    let transaction_hash = fixture.send(TxKind::Call(CONTRACT), &[], GAS_LIMIT, U256::ZERO)?;

    let trace =
        fixture.trace_transaction(transaction_hash, Some(json!({ "tracer": "callTracer" })))?;

    assert_eq!(field(&trace, "type"), &json!(TraceCallKind::Call.to_str()));
    assert_eq!(
        field(&trace, "from"),
        &serde_json::to_value(fixture.sender)?
    );
    assert_eq!(field(&trace, "to"), &serde_json::to_value(CONTRACT)?);
    let calls = field(&trace, "calls")
        .as_array()
        .expect("`calls` is an array");
    let callees: Vec<&Value> = calls.iter().map(|call| field(call, "to")).collect();
    assert_eq!(callees, [&serde_json::to_value(CALLEE)?]);
    assert_eq!(trace.get("structLogs"), None);

    Ok(())
}

#[test]
fn trace_call_rejects_unknown_tracer_as_js_tracer() -> anyhow::Result<()> {
    let mut fixture = TraceFixture::new(&[])?;

    let error = fixture
        .trace_call(
            Some(RECIPIENT),
            &[],
            None,
            Some(json!({ "tracer": UNKNOWN_TRACER })),
        )
        .expect_err("an unknown tracer should be rejected");

    assert_js_tracer_rejected(error);

    Ok(())
}

#[test]
fn trace_transaction_rejects_unknown_tracer_as_js_tracer() -> anyhow::Result<()> {
    let mut fixture = TraceFixture::new(&[])?;
    let transaction_hash = fixture.send(
        TxKind::Call(RECIPIENT),
        &[],
        TRANSACTION_BASE_GAS,
        U256::from(TRANSFER_VALUE),
    )?;

    let error = fixture
        .trace_transaction(transaction_hash, Some(json!({ "tracer": UNKNOWN_TRACER })))
        .expect_err("an unknown tracer should be rejected");

    assert_js_tracer_rejected(error);

    Ok(())
}

#[test]
fn trace_transaction_rejects_unknown_hash() -> anyhow::Result<()> {
    let mut fixture = TraceFixture::new(&[])?;

    let error = fixture
        .trace_transaction(B256::ZERO, None)
        .expect_err("an unknown hash should be rejected");

    assert!(
        matches!(error, ProviderError::InvalidInput(_)),
        "unexpected error: {error:?}"
    );
    let wire_error = jsonrpc::Error::from(error);
    assert_eq!(wire_error.code, INVALID_INPUT);
    assert_eq!(
        wire_error.message,
        format!(
            "Unable to find a block containing transaction {}",
            B256::ZERO
        )
    );

    Ok(())
}

#[test]
fn trace_call_defaults_to_latest_block() -> anyhow::Result<()> {
    let mut fixture = TraceFixture::new(&[(CONTRACT, &number_then(&READ_THEN_WRITE_CODE))])?;
    fixture.send(TxKind::Call(CONTRACT), &[], GAS_LIMIT, U256::ZERO)?;
    let latest_number = fixture.last_block_number();

    let without_block = fixture.trace_call(Some(CONTRACT), &[], None, None)?;
    let latest = fixture.trace_call(Some(CONTRACT), &[], Some(BlockSpec::latest()), None)?;

    assert_eq!(
        field(
            step_after_op(&without_block, OpCode::NUMBER.as_str()),
            "stack"
        ),
        &json!([format!("{latest_number:#x}")])
    );
    assert_eq!(
        field(
            step_with_op(&without_block, OpCode::SLOAD.as_str()),
            "storage"
        ),
        &storage(&[(SLOT, STORED_VALUE)])
    );
    assert_eq!(without_block, latest);

    Ok(())
}

#[test]
fn trace_call_without_recipient_traces_creation() -> anyhow::Result<()> {
    let mut fixture = TraceFixture::new(&[])?;

    let trace = fixture.trace_call(None, &EMPTY_INIT_CODE, None, None)?;

    assert_eq!(ops(&trace), op_names(&EMPTY_INIT_CODE));

    Ok(())
}

#[test]
fn trace_call_does_not_persist_state() -> anyhow::Result<()> {
    let mut fixture = TraceFixture::new(&[(CONTRACT, &READ_THEN_WRITE_CODE)])?;

    fixture.trace_call(Some(CONTRACT), &[], None, None)?;
    let trace = fixture.trace_call(Some(CONTRACT), &[], None, None)?;

    assert_eq!(
        field(step_with_op(&trace, OpCode::SLOAD.as_str()), "storage"),
        &storage(&[(SLOT, UNSET_VALUE)])
    );

    Ok(())
}

#[test]
fn trace_and_struct_logs_have_exactly_the_spec_fields() -> anyhow::Result<()> {
    let mut fixture = TraceFixture::new(&[
        (CONTRACT, &caller_code(CallKind::Call)),
        (CALLEE, &INFINITE_LOOP_CODE),
    ])?;
    let transaction_hash = fixture.send(
        TxKind::Call(CONTRACT),
        &[],
        CALLING_LOOP_GAS_LIMIT,
        U256::ZERO,
    )?;

    let trace = fixture.trace_transaction(
        transaction_hash,
        Some(json!({ "enableMemory": true, "enableReturnData": true })),
    )?;

    assert_eq!(
        keys(&trace),
        BTreeSet::from(TRACE_FIELDS),
        "the trace should carry only the OpcodeTransactionTrace fields"
    );
    let struct_log_fields = BTreeSet::from(STRUCT_LOG_FIELDS);
    let mut step_fields = BTreeSet::new();
    for step in struct_logs(&trace) {
        for key in keys(step) {
            assert!(
                struct_log_fields.contains(key),
                "`{key}` is not a spec field in {step}"
            );
            step_fields.insert(key);
        }
    }
    assert_eq!(
        step_fields, struct_log_fields,
        "the steps should together exercise every StructLog field"
    );

    Ok(())
}
