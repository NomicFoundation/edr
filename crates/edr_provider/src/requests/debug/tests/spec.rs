//! Tests per requirement of the execution-apis opcode logger and of the
//! method clauses in `trace.yaml`.

use edr_receipt::ReceiptTrait as _;

use super::*;

/// The local account that sends the second transaction of the traced block.
const OTHER_SENDER_INDEX: usize = 1;
/// The index of the second transaction in its block.
const SECOND_TRANSACTION_INDEX: u64 = 1;

/// The spec requires `pc`, `op`, `gas`, `gasCost` and `depth`, with integer
/// `gas` and `gasCost`.
#[test]
fn struct_log_has_the_required_fields() -> anyhow::Result<()> {
    let trace = trace_read_then_write(None)?;

    assert_has_steps(&trace);
    for step in struct_logs(&trace) {
        for key in ["depth", "op", "pc"] {
            assert!(step.get(key).is_some(), "`{key}` is missing in {step}");
        }
        integer_field(step, "gas");
        integer_field(step, "gasCost");
    }

    Ok(())
}

/// For `disableStack`, "Setting to true disables stack capture ... Default:
/// false", and `stack` is "absent when disableStack is true".
#[test]
fn stack_is_present_by_default() -> anyhow::Result<()> {
    let trace = trace_read_then_write(None)?;

    assert_has_steps(&trace);
    for step in struct_logs(&trace) {
        assert!(step.get("stack").is_some(), "`stack` is missing in {step}");
    }

    Ok(())
}

/// `error` "MUST be absent when no error occurred", and `memory` and
/// `returnData` are absent while `enableMemory` and `enableReturnData` are
/// false.
#[test]
fn struct_log_omits_error_memory_and_return_data_by_default() -> anyhow::Result<()> {
    let trace = trace_read_then_write(None)?;

    assert_has_steps(&trace);
    for step in struct_logs(&trace) {
        for key in ["error", "memory", "returnData"] {
            assert_eq!(step.get(key), None, "`{key}` is present in {step}");
        }
    }

    Ok(())
}

/// `pc` is "The program counter position of this opcode in the contract
/// bytecode".
#[test]
fn pc_is_the_byte_offset_of_each_executed_opcode() -> anyhow::Result<()> {
    let trace = trace_read_then_write(None)?;

    let pcs: Vec<u64> = struct_logs(&trace)
        .iter()
        .map(|step| integer_field(step, "pc"))
        .collect();

    assert_eq!(pcs, opcode_offsets(&READ_THEN_WRITE_CODE));

    Ok(())
}

/// `gas` is "The amount of gas remaining before executing this opcode". On the
/// first step that is the gas limit minus the intrinsic gas.
#[test]
fn gas_of_the_first_step_is_the_gas_limit_minus_intrinsic_gas() -> anyhow::Result<()> {
    let trace = trace_read_then_write(None)?;

    assert_eq!(
        integer_field(nth_step(&trace, 0), "gas"),
        GAS_LIMIT - TRANSACTION_BASE_GAS
    );

    Ok(())
}

/// `gasCost` is "the amount that will be deducted from the remaining gas
/// assuming successful execution".
#[test]
fn gas_drops_by_the_previous_gas_cost() -> anyhow::Result<()> {
    let trace = trace_read_then_write(None)?;

    assert_has_steps(&trace);
    let steps = struct_logs(&trace);
    for (step, next) in steps.iter().zip(steps.iter().skip(1)) {
        assert_eq!(
            integer_field(next, "gas"),
            integer_field(step, "gas") - integer_field(step, "gasCost"),
            "unexpected gas after {step}"
        );
    }

    Ok(())
}

/// `gasCost` "includes dynamic costs such as memory expansion and storage
/// access charges (e.g. cold vs. warm SLOAD/SSTORE)".
#[test]
fn gas_cost_includes_the_cold_sload_charge() -> anyhow::Result<()> {
    let trace = trace_read_then_write(None)?;

    assert_eq!(
        integer_field(step_with_op(&trace, OpCode::SLOAD.as_str()), "gasCost"),
        COLD_SLOAD_COST
    );

    Ok(())
}

/// `refund` is "The accumulated gas refund counter at this execution step",
/// which clients "MUST include ... when it is non-zero". The test pins the
/// [EIP-2200] restore amount, which the spec leaves to the client.
///
/// [EIP-2200]: https://eips.ethereum.org/EIPS/eip-2200
#[test]
fn refund_is_the_restore_refund_after_an_sstore_restores_the_original_value() -> anyhow::Result<()>
{
    let trace = trace_contract_transaction(&[(CONTRACT, &WRITE_THEN_RESTORE_CODE)], None)?;

    let restore_index = struct_logs(&trace)
        .iter()
        .rposition(|step| op(step) == OpCode::SSTORE.as_str())
        .expect("the restoring `SSTORE` step exists");

    assert_eq!(
        integer_field(nth_step(&trace, restore_index + 1), "refund"),
        RESTORE_REFUND
    );

    Ok(())
}

/// The trace's `gas` is the "Total amount of gas consumed by the
/// transaction". Here that is the gas used after the capped refund.
#[test]
fn trace_gas_is_the_gas_used_after_the_capped_refund() -> anyhow::Result<()> {
    let trace = trace_contract_transaction(&[(CONTRACT, &WRITE_THEN_RESTORE_CODE)], None)?;

    assert_eq!(integer_field(&trace, "gas"), WRITE_THEN_RESTORE_GAS_USED);

    Ok(())
}

/// `stack` is "ordered from bottom (index 0) to top", and "Each value is a
/// 0x-prefixed uint256 quantity".
#[test]
fn stack_holds_uint256_quantities_from_bottom_to_top() -> anyhow::Result<()> {
    let trace = trace_read_then_write(None)?;

    let sstore = step_with_op(&trace, OpCode::SSTORE.as_str());

    assert_eq!(
        field(sstore, "stack"),
        &json!([format!("{STORED_VALUE:#x}"), format!("{SLOT:#x}")])
    );

    Ok(())
}

/// Clients "SHOULD include this field at every step even when zero".
#[test]
fn refund_is_an_integer_on_every_step_including_zero_refund() -> anyhow::Result<()> {
    let trace = trace_read_then_write(None)?;

    assert_has_steps(&trace);
    for step in struct_logs(&trace) {
        assert!(field(step, "refund").is_u64(), "unexpected step: {step}");
    }
    assert_eq!(field(nth_step(&trace, 0), "refund"), &json!(NO_REFUND));

    Ok(())
}

/// `storage` is "A cumulative snapshot of all storage slots accessed or
/// modified by the current contract up to and including this step".
#[test]
fn storage_is_a_cumulative_snapshot_of_slots_accessed_or_modified() -> anyhow::Result<()> {
    let trace = trace_contract_transaction(&[(CONTRACT, &READ_THEN_WRITE_OTHER_SLOT_CODE)], None)?;

    let sstore = step_with_op(&trace, OpCode::SSTORE.as_str());

    assert_eq!(
        field(sstore, "storage"),
        &storage(&[(SLOT, UNSET_VALUE), (CALLER_SLOT, STORED_VALUE)])
    );

    Ok(())
}

/// `storage` "is only populated at SLOAD and SSTORE opcodes" and "MUST be
/// absent" elsewhere.
#[test]
fn storage_is_present_only_at_sload_and_sstore_steps() -> anyhow::Result<()> {
    let trace = trace_read_then_write(None)?;

    let ops_with_storage: Vec<&str> = struct_logs(&trace)
        .iter()
        .filter(|step| step.get("storage").is_some())
        .map(op)
        .collect();

    assert_eq!(
        ops_with_storage,
        [OpCode::SLOAD.as_str(), OpCode::SSTORE.as_str()]
    );

    Ok(())
}

/// `storage` "is only populated at SLOAD and SSTORE opcodes". The snapshot
/// does not depend on the access being cold.
#[test]
fn warm_sload_step_has_a_storage_snapshot() -> anyhow::Result<()> {
    let trace = trace_contract_transaction(&[(CONTRACT, &READ_TWICE_CODE)], None)?;

    let sloads: Vec<&Value> = struct_logs(&trace)
        .iter()
        .filter(|step| op(step) == OpCode::SLOAD.as_str())
        .collect();
    let [cold_sload, warm_sload] = sloads.as_slice() else {
        panic!("two `SLOAD` steps should exist in {trace}");
    };

    assert_eq!(
        integer_field(warm_sload, "gasCost"),
        WARM_STORAGE_READ_GAS,
        "the second `SLOAD` should be warm"
    );
    for sload in [cold_sload, warm_sload] {
        assert_eq!(
            field(sload, "storage"),
            &storage(&[(SLOT, UNSET_VALUE)]),
            "unexpected step: {sload}"
        );
    }

    Ok(())
}

/// Storage keys are "zero-padded 32-byte storage slot indices", and "for
/// SSTORE, the value reflects the new value just written".
#[test]
fn sstore_storage_has_zero_padded_32_byte_slot_and_the_value_just_written() -> anyhow::Result<()> {
    let trace = trace_read_then_write(None)?;

    let sstore = step_with_op(&trace, OpCode::SSTORE.as_str());

    assert_eq!(field(sstore, "storage"), &storage(&[(SLOT, STORED_VALUE)]));

    Ok(())
}

/// `storage` covers the slots of "the current contract", so the caller's and
/// the called contract's snapshots stay apart.
#[test]
fn storage_holds_only_slots_of_the_current_contract() -> anyhow::Result<()> {
    let trace = trace_transaction_calling(&WRITER_CODE, None)?;

    let callee_sstore = struct_logs(&trace)
        .iter()
        .find(|step| {
            op(step) == OpCode::SSTORE.as_str() && field(step, "depth") == &json!(CALLEE_DEPTH)
        })
        .unwrap_or_else(|| panic!("the callee's `SSTORE` step should exist in {trace}"));
    let sload = step_with_op(&trace, OpCode::SLOAD.as_str());

    assert_eq!(
        field(callee_sstore, "storage"),
        &storage(&[(WRITER_SLOT, STORED_VALUE)])
    );
    assert_eq!(
        field(sload, "storage"),
        &storage(&[(CALLER_SLOT, STORED_VALUE)])
    );

    Ok(())
}

/// `storage` covers the slots of "the current contract". A `DELEGATECALL`
/// executes in the caller's context, so its writes join the caller's
/// snapshot.
#[test]
fn storage_includes_slots_a_delegatecall_wrote_for_the_caller() -> anyhow::Result<()> {
    let trace = trace_contract_transaction(
        &[
            (CONTRACT, &caller_code(CallKind::DelegateCall)),
            (CALLEE, &WRITER_CODE),
        ],
        None,
    )?;

    let sload = step_with_op(&trace, OpCode::SLOAD.as_str());

    assert_eq!(
        field(sload, "storage"),
        &storage(&[(CALLER_SLOT, STORED_VALUE), (WRITER_SLOT, STORED_VALUE)])
    );

    Ok(())
}

/// `structLogs` "Will be empty for simple ETH transfers", and `returnValue`
/// "MUST be \"0x\"" when no data was returned. `gas` is the "Total amount of
/// gas consumed".
#[test]
fn simple_transfer_trace_has_no_struct_logs_and_returns_0x() -> anyhow::Result<()> {
    let mut fixture = TraceFixture::new(&[])?;
    let transaction_hash = fixture.send(
        TxKind::Call(RECIPIENT),
        &[],
        TRANSACTION_BASE_GAS,
        U256::from(TRANSFER_VALUE),
    )?;

    let trace = fixture.trace_transaction(transaction_hash, None)?;

    assert_eq!(
        trace,
        json!({
            "failed": false,
            "gas": TRANSACTION_BASE_GAS,
            "returnValue": "0x",
            "structLogs": [],
        })
    );

    Ok(())
}

/// `failed` is whether the execution "failed (reverted or ran out of gas)",
/// and `returnValue` is "The data returned by the transaction". For a revert
/// that is the revert data.
#[test]
fn failed_is_true_and_return_value_holds_revert_data() -> anyhow::Result<()> {
    let trace = trace_contract_transaction(&[(CONTRACT, &return_word_code(opcode::REVERT))], None)?;

    assert_eq!(field(&trace, "failed"), &json!(true));
    assert_eq!(field(&trace, "returnValue"), &json!(word(STORED_VALUE)));

    Ok(())
}

/// `failed` is false for a successful execution, and `returnValue` is "The
/// data returned by the transaction".
#[test]
fn successful_call_has_failed_false_and_its_output_as_return_value() -> anyhow::Result<()> {
    let trace = trace_return_word(None)?;

    assert_eq!(field(&trace, "failed"), &json!(false));
    assert_eq!(field(&trace, "returnValue"), &json!(word(STORED_VALUE)));

    Ok(())
}

/// `failed` covers a transaction that "reverted or ran out of gas", and `gas`
/// is the "Total amount of gas consumed". A precompile executes no opcode, so
/// "One entry is emitted per opcode executed" leaves no struct logs.
#[test]
fn precompile_halt_consumes_the_gas_limit_without_struct_logs() -> anyhow::Result<()> {
    let mut fixture = TraceFixture::new(&[])?;
    let transaction_hash = fixture.send(
        TxKind::Call(ECRECOVER),
        &[],
        PRECOMPILE_GAS_LIMIT,
        U256::ZERO,
    )?;

    let trace = fixture.trace_transaction(transaction_hash, None)?;

    assert_eq!(field(&trace, "failed"), &json!(true));
    assert_eq!(field(&trace, "gas"), &json!(PRECOMPILE_GAS_LIMIT));
    assert_eq!(struct_logs(&trace), &[] as &[Value]);

    Ok(())
}

/// "One entry is emitted per opcode executed."
#[test]
fn one_struct_log_is_emitted_per_executed_opcode() -> anyhow::Result<()> {
    let trace = trace_read_then_write(None)?;

    assert_eq!(ops(&trace), op_names(&READ_THEN_WRITE_CODE));

    Ok(())
}

/// "One entry is emitted per opcode executed", so a lone `STOP` yields one.
#[test]
fn stop_only_contract_has_one_struct_log() -> anyhow::Result<()> {
    let trace = trace_contract_transaction(&[(CONTRACT, &[opcode::STOP])], None)?;

    assert_eq!(ops(&trace), op_names(&[opcode::STOP]));

    Ok(())
}

/// `depth` "Starts at 1 for the outermost call", which for a creation is the
/// init code.
#[test]
fn depth_of_contract_creation_init_code_is_the_outermost_depth() -> anyhow::Result<()> {
    let mut fixture = TraceFixture::new(&[])?;
    let transaction_hash = fixture.send(TxKind::Create, &EMPTY_INIT_CODE, GAS_LIMIT, U256::ZERO)?;

    let trace = fixture.trace_transaction(transaction_hash, None)?;

    let depths: Vec<&Value> = struct_logs(&trace)
        .iter()
        .map(|step| field(step, "depth"))
        .collect();
    assert_eq!(
        depths,
        vec![&json!(TOP_LEVEL_DEPTH); opcode_offsets(&EMPTY_INIT_CODE).len()]
    );

    Ok(())
}

/// `depth` is "The call stack depth at this execution step". A called frame is
/// one deeper than its caller.
#[test]
fn depth_is_one_more_in_a_called_frame() -> anyhow::Result<()> {
    let trace = trace_transaction_calling(&WRITER_CODE, None)?;

    assert_eq!(
        depths_of_op(&trace, OpCode::SSTORE.as_str()),
        [&json!(TOP_LEVEL_DEPTH), &json!(CALLEE_DEPTH)]
    );

    Ok(())
}

/// `memory` holds "32-byte chunks", each "a 0x-prefixed, zero-padded 32-byte
/// hex word", when `enableMemory` is true.
#[test]
fn enable_memory_adds_zero_padded_32_byte_memory_words() -> anyhow::Result<()> {
    let trace = trace_return_word(Some(json!({ "enableMemory": true })))?;

    let step = step_after_op(&trace, OpCode::MSTORE.as_str());

    assert_eq!(field(step, "memory"), &json!([word(STORED_VALUE)]));

    Ok(())
}

/// "When true, the stack field is omitted from each StructLog entry."
#[expect(
    clippy::doc_markdown,
    reason = "the spec quote names StructLog verbatim"
)]
#[test]
fn stack_is_absent_when_disable_stack_is_true() -> anyhow::Result<()> {
    let trace = trace_read_then_write(Some(json!({ "disableStack": true })))?;

    assert_has_steps(&trace);
    for step in struct_logs(&trace) {
        assert_eq!(step.get("stack"), None, "unexpected step: {step}");
    }

    Ok(())
}

/// "When true, the storage field is omitted from each StructLog entry."
#[expect(
    clippy::doc_markdown,
    reason = "the spec quote names StructLog verbatim"
)]
#[test]
fn storage_is_absent_when_disable_storage_is_true() -> anyhow::Result<()> {
    let trace = trace_read_then_write(Some(json!({ "disableStorage": true })))?;

    assert_has_steps(&trace);
    for step in struct_logs(&trace) {
        assert_eq!(step.get("storage"), None, "unexpected step: {step}");
    }

    Ok(())
}

/// `returnData` is "The return data from the most recent call at this step"
/// when `enableReturnData` is true.
#[test]
fn enable_return_data_adds_the_most_recent_call_return_data() -> anyhow::Result<()> {
    let trace = trace_transaction_calling(
        &return_word_code(opcode::RETURN),
        Some(json!({ "enableReturnData": true })),
    )?;

    assert_eq!(
        field(step_after_op(&trace, OpCode::CALL.as_str()), "returnData"),
        &json!(word(STORED_VALUE))
    );

    Ok(())
}

/// "When absent or empty, the opcode (struct) logger is used."
#[test]
fn empty_tracer_selects_the_opcode_logger() -> anyhow::Result<()> {
    let trace = trace_read_then_write(Some(json!({ "tracer": "" })))?;

    assert_eq!(ops(&trace), op_names(&READ_THEN_WRITE_CODE));

    Ok(())
}

/// `limit` is the "Maximum number of opcode steps to capture".
#[test]
fn limit_keeps_only_the_first_steps() -> anyhow::Result<()> {
    let trace = trace_read_then_write(Some(json!({ "limit": STEP_LIMIT })))?;

    let expected: Vec<&str> = op_names(&READ_THEN_WRITE_CODE)
        .into_iter()
        .take(STEP_LIMIT)
        .collect();
    assert_eq!(ops(&trace), expected);

    Ok(())
}

/// "When the limit is reached, execution continues but no further StructLog
/// entries are recorded."
#[expect(
    clippy::doc_markdown,
    reason = "the spec quote names StructLog verbatim"
)]
#[test]
fn limit_does_not_stop_execution() -> anyhow::Result<()> {
    let limited = trace_read_then_write(Some(json!({ "limit": STEP_LIMIT })))?;
    let unlimited = trace_read_then_write(None)?;

    for key in ["failed", "gas"] {
        assert_eq!(
            field(&limited, key),
            field(&unlimited, key),
            "`{key}` differs"
        );
    }

    Ok(())
}

/// For `limit`, "Zero means no limit".
#[test]
fn limit_zero_means_unlimited() -> anyhow::Result<()> {
    let trace = trace_read_then_write(Some(json!({ "limit": 0 })))?;

    assert_eq!(ops(&trace), op_names(&READ_THEN_WRITE_CODE));

    Ok(())
}

/// `debug_traceTransaction` executes the transaction "replaying all prior
/// transactions in the same block as necessary to reconstruct the correct
/// pre-execution state".
#[test]
fn trace_transaction_replays_the_preceding_transactions_of_its_block() -> anyhow::Result<()> {
    let mut fixture = TraceFixture::new(&[(CONTRACT, &READ_THEN_WRITE_CODE)])?;
    fixture.set_auto_mining(false);
    fixture.send(TxKind::Call(CONTRACT), &[], GAS_LIMIT, U256::ZERO)?;
    // A second sender keeps the nonce valid without the replay.
    fixture.sender = fixture.provider.nth_local_account(OTHER_SENDER_INDEX)?;
    let second_hash = fixture.send(TxKind::Call(CONTRACT), &[], GAS_LIMIT, U256::ZERO)?;
    fixture.mine()?;
    let second_receipt = fixture
        .provider
        .provider_data
        .transaction_receipt(&second_hash)?
        .expect("the second transaction is mined");
    assert_eq!(
        second_receipt.transaction_index(),
        SECOND_TRANSACTION_INDEX,
        "both transactions should be mined into one block"
    );

    let trace = fixture.trace_transaction(second_hash, None)?;

    assert_eq!(
        field(step_with_op(&trace, OpCode::SLOAD.as_str()), "storage"),
        &storage(&[(SLOT, STORED_VALUE)])
    );

    Ok(())
}
