//!  This file includes Solidity tracing heuristics for solc starting with
//! version  0.6.9.
//!
//!  This solc version introduced a significant change to how sourcemaps are
//!  handled for inline yul/internal functions. These were mapped to the
//!  unmapped/-1 file before, which lead to many unmapped reverts. Now, they are
//!  mapped to the part of the Solidity source that lead to their inlining.
//!
//!  This change is a very positive change, as errors would point to the correct
//!  line by default. The only problem is that we used to rely very heavily on
//!  unmapped reverts to decide when our error detection heuristics were to be
//!  run. In fact, these heuristics were first introduced because of unmapped
//!  reverts.
//!
//!  Instead of synthetically completing stack traces when unmapped reverts
//! occur, we now start from complete stack traces and adjust them if we can
//! provide more meaningful errors.

use edr_evm::interpreter::OpCode;
use napi::{
    bindgen_prelude::{Either24, Either4},
    Either,
};
use semver::Version;

use super::{
    message_trace::{CallMessageTrace, CreateMessageTrace, EvmStep},
    solidity_stack_trace::{
        InvalidParamsErrorStackTraceEntry, NonContractAccountCalledErrorStackTraceEntry,
        RevertErrorStackTraceEntry, SolidityStackTrace, StackTraceEntryTypeConst,
    },
};

const FIRST_SOLC_VERSION_WITH_MAPPED_SMALL_INTERNAL_FUNCTIONS: Version = Version::new(0, 6, 9);

pub fn stack_trace_may_require_adjustments(
    stacktrace: &SolidityStackTrace,
    decoded_trace: Either<&CallMessageTrace, &CreateMessageTrace>,
) -> bool {
    let bytecode = match &decoded_trace {
        Either::A(create) => &create.bytecode,
        Either::B(call) => &call.bytecode,
    };
    let bytecode = bytecode.as_ref().expect("JS code asserts");

    let Some(last_frame) = stacktrace.last() else {
        return false;
    };

    if let Either24::E(last_frame @ RevertErrorStackTraceEntry { .. }) = last_frame {
        return !last_frame.is_invalid_opcode_error
            && last_frame.return_data.is_empty()
            && Version::parse(&bytecode.compiler_version)
                .map(|version| version >= FIRST_SOLC_VERSION_WITH_MAPPED_SMALL_INTERNAL_FUNCTIONS)
                .unwrap_or(false);
    }

    false
}

pub fn adjust_stack_trace(
    mut stacktrace: SolidityStackTrace,
    decoded_trace: Either<&CallMessageTrace, &CreateMessageTrace>,
) -> napi::Result<SolidityStackTrace> {
    let Some(Either24::E(revert @ RevertErrorStackTraceEntry { .. })) = stacktrace.last() else {
        unreachable!("JS code asserts that; it's only used immediately after we check with `stack_trace_may_require_adjustments` that the last frame is a revert frame");
    };

    // Replace the last revert frame with an adjusted frame if needed
    if is_non_contract_account_called_error(decoded_trace)? {
        let last_revert_frame_source_reference = revert.source_reference.clone();
        stacktrace.pop();
        stacktrace.push(
            NonContractAccountCalledErrorStackTraceEntry {
                type_: StackTraceEntryTypeConst,
                source_reference: last_revert_frame_source_reference,
            }
            .into(),
        );
        return Ok(stacktrace);
    }

    if is_constructor_invalid_params_error(decoded_trace)? {
        let last_revert_frame_source_reference = revert.source_reference.clone();
        stacktrace.pop();
        stacktrace.push(
            InvalidParamsErrorStackTraceEntry {
                type_: StackTraceEntryTypeConst,
                source_reference: last_revert_frame_source_reference,
            }
            .into(),
        );
        return Ok(stacktrace);
    }

    if is_call_invalid_params_error(decoded_trace)? {
        let last_revert_frame_source_reference = revert.source_reference.clone();
        stacktrace.pop();
        stacktrace.push(
            InvalidParamsErrorStackTraceEntry {
                type_: StackTraceEntryTypeConst,
                source_reference: last_revert_frame_source_reference,
            }
            .into(),
        );

        return Ok(stacktrace);
    }

    Ok(stacktrace)
}

fn is_non_contract_account_called_error(
    decoded_trace: Either<&CallMessageTrace, &CreateMessageTrace>,
) -> napi::Result<bool> {
    match_opcodes(
        decoded_trace,
        -9,
        &[
            OpCode::EXTCODESIZE,
            OpCode::ISZERO,
            OpCode::DUP1,
            OpCode::ISZERO,
        ],
    )
}

fn is_constructor_invalid_params_error(
    decoded_trace: Either<&CallMessageTrace, &CreateMessageTrace>,
) -> napi::Result<bool> {
    Ok(match_opcodes(decoded_trace, -20, &[OpCode::CODESIZE])?
        && match_opcodes(decoded_trace, -15, &[OpCode::CODECOPY])?
        && match_opcodes(decoded_trace, -7, &[OpCode::LT, OpCode::ISZERO])?)
}

fn is_call_invalid_params_error(
    decoded_trace: Either<&CallMessageTrace, &CreateMessageTrace>,
) -> napi::Result<bool> {
    Ok(match_opcodes(decoded_trace, -11, &[OpCode::CALLDATASIZE])?
        && match_opcodes(decoded_trace, -7, &[OpCode::LT, OpCode::ISZERO])?)
}

fn match_opcodes(
    decoded_trace: Either<&CallMessageTrace, &CreateMessageTrace>,
    first_step_index: i32,
    opcodes: &[OpCode],
) -> napi::Result<bool> {
    let (bytecode, steps) = match &decoded_trace {
        Either::A(call) => (&call.bytecode, &call.steps),
        Either::B(create) => (&create.bytecode, &create.steps),
    };
    let bytecode = bytecode.as_ref().expect("JS code asserts");

    // If the index is negative, we start from the end of the trace,
    // just like in the original JS code
    let mut index = match first_step_index {
        0.. => first_step_index as usize,
        ..=-1 if first_step_index.abs() < steps.len() as i32 => {
            (steps.len() as i32 + first_step_index) as usize
        }
        // Out of bounds
        _ => return Ok(false),
    };

    for opcode in opcodes {
        let Some(Either4::A(EvmStep { pc })) = steps.get(index) else {
            return Ok(false);
        };

        let instruction = bytecode.get_instruction(*pc)?;

        if instruction.opcode != *opcode {
            return Ok(false);
        }

        index += 1;
    }

    Ok(true)
}
