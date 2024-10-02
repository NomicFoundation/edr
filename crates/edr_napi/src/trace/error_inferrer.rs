use std::{borrow::Cow, collections::HashSet};

use alloy_dyn_abi::{DynSolValue, JsonAbiExt};
use edr_evm::{hex, interpreter::OpCode};
use edr_solidity::build_model::{
    Bytecode, ContractFunction, ContractFunctionType, ContractKind, Instruction, JumpType,
    SourceLocation,
};
use napi::{
    bindgen_prelude::{BigInt, Either24, Either3, Either4},
    Either,
};
use semver::{Version, VersionReq};

use super::{
    exit::ExitCode,
    message_trace::{CallMessageTrace, CreateMessageTrace, EvmStep, PrecompileMessageTrace},
    model::ContractFunctionType as ContractFunctionTypeNapi,
    return_data::ReturnData,
    solidity_stack_trace::{
        CallFailedErrorStackTraceEntry, CallstackEntryStackTraceEntry, CustomErrorStackTraceEntry,
        FallbackNotPayableErrorStackTraceEntry, InternalFunctionCallStackEntry,
        InvalidParamsErrorStackTraceEntry, NonContractAccountCalledErrorStackTraceEntry,
        PanicErrorStackTraceEntry, SolidityStackTrace, SolidityStackTraceEntry,
        SolidityStackTraceEntryExt, SourceReference, UnmappedSolc063RevertErrorStackTraceEntry,
    },
};
use crate::trace::solidity_stack_trace::{
    ContractCallRunOutOfGasError, ContractTooLargeErrorStackTraceEntry,
    DirectLibraryCallErrorStackTraceEntry, FallbackNotPayableAndNoReceiveErrorStackTraceEntry,
    FunctionNotPayableErrorStackTraceEntry, MissingFallbackOrReceiveErrorStackTraceEntry,
    OtherExecutionErrorStackTraceEntry, ReturndataSizeErrorStackTraceEntry,
    RevertErrorStackTraceEntry, StackTraceEntryTypeConst,
    UnrecognizedFunctionWithoutFallbackErrorStackTraceEntry, CONSTRUCTOR_FUNCTION_NAME,
    FALLBACK_FUNCTION_NAME, RECEIVE_FUNCTION_NAME,
};

/// Specifies whether a heuristic was applied and modified the stack trace.
///
/// Think of it as happy [`Result`] - the [`Heuristic::Hit`] should be
/// propagated to the caller.
#[must_use]
pub enum Heuristic {
    /// The heuristic was applied and modified the stack trace.
    Hit(SolidityStackTrace),
    /// The heuristic did not apply; the stack trace is unchanged.
    Miss(SolidityStackTrace),
}

const FIRST_SOLC_VERSION_CREATE_PARAMS_VALIDATION: Version = Version::new(0, 5, 9);
const FIRST_SOLC_VERSION_RECEIVE_FUNCTION: Version = Version::new(0, 6, 0);
const FIRST_SOLC_VERSION_WITH_UNMAPPED_REVERTS: &str = "0.6.3";

/// Port of `SubmessageData` from Hardhat to Rust
// However, it borrows the traces (instead of copying them) because the traces
// do not implement `Clone` due to inner references
pub struct SubmessageDataRef<'a> {
    pub message_trace:
        Either3<&'a PrecompileMessageTrace, &'a CallMessageTrace, &'a CreateMessageTrace>,
    pub stacktrace: SolidityStackTrace,
    pub step_index: u32,
}
#[derive(Default)]
pub struct ErrorInferrer;

impl ErrorInferrer {
    pub fn infer_before_tracing_call_message(
        trace: &CallMessageTrace,
    ) -> napi::Result<Option<SolidityStackTrace>> {
        if Self::is_direct_library_call(trace)? {
            return Ok(Some(Self::get_direct_library_call_error_stack_trace(
                trace,
            )?));
        }

        let bytecode = trace
            .bytecode
            .as_ref()
            .expect("The TS code type-checks this to always have bytecode");
        let contract = bytecode.contract.borrow();

        let called_function = contract
            .get_function_from_selector(trace.calldata.get(..4).unwrap_or(&trace.calldata[..]));

        if let Some(called_function) = called_function {
            if Self::is_function_not_payable_error(trace, called_function)? {
                return Ok(Some(vec![FunctionNotPayableErrorStackTraceEntry {
                    type_: StackTraceEntryTypeConst,
                    source_reference: Self::get_function_start_source_reference(
                        Either::A(trace),
                        called_function,
                    )?,
                    value: trace.value.clone(),
                }
                .into()]));
            }
        }

        let called_function = called_function.map(AsRef::as_ref);

        if Self::is_missing_function_and_fallback_error(trace, called_function)? {
            let source_reference =
                Self::get_contract_start_without_function_source_reference(Either::A(trace))?;

            if Self::empty_calldata_and_no_receive(trace)? {
                return Ok(Some(vec![MissingFallbackOrReceiveErrorStackTraceEntry {
                    type_: StackTraceEntryTypeConst,
                    source_reference,
                }
                .into()]));
            }

            return Ok(Some(vec![
                UnrecognizedFunctionWithoutFallbackErrorStackTraceEntry {
                    type_: StackTraceEntryTypeConst,
                    source_reference,
                }
                .into(),
            ]));
        }

        if Self::is_fallback_not_payable_error(trace, called_function)? {
            let source_reference = Self::get_fallback_start_source_reference(trace)?;

            if Self::empty_calldata_and_no_receive(trace)? {
                return Ok(Some(vec![
                    FallbackNotPayableAndNoReceiveErrorStackTraceEntry {
                        type_: StackTraceEntryTypeConst,
                        source_reference,
                        value: trace.value.clone(),
                    }
                    .into(),
                ]));
            }

            return Ok(Some(vec![FallbackNotPayableErrorStackTraceEntry {
                type_: StackTraceEntryTypeConst,
                source_reference,
                value: trace.value.clone(),
            }
            .into()]));
        }

        Ok(None)
    }

    pub fn infer_before_tracing_create_message(
        trace: &CreateMessageTrace,
    ) -> napi::Result<Option<SolidityStackTrace>> {
        if Self::is_constructor_not_payable_error(trace)? {
            return Ok(Some(vec![FunctionNotPayableErrorStackTraceEntry {
                type_: StackTraceEntryTypeConst,
                source_reference: Self::get_constructor_start_source_reference(trace)?,
                value: trace.value.clone(),
            }
            .into()]));
        }

        if Self::is_constructor_invalid_arguments_error(trace)? {
            return Ok(Some(vec![InvalidParamsErrorStackTraceEntry {
                type_: StackTraceEntryTypeConst,
                source_reference: Self::get_constructor_start_source_reference(trace)?,
            }
            .into()]));
        }

        Ok(None)
    }

    pub fn infer_after_tracing(
        trace: Either<&CallMessageTrace, &CreateMessageTrace>,
        stacktrace: SolidityStackTrace,
        function_jumpdests: &[&Instruction],
        jumped_into_function: bool,
        last_submessage_data: Option<SubmessageDataRef<'_>>,
    ) -> napi::Result<SolidityStackTrace> {
        /// Convenience macro to early return the result if a heuristic hits.
        macro_rules! return_if_hit {
            ($heuristic: expr) => {
                match $heuristic {
                    Heuristic::Hit(stacktrace) => return Ok(stacktrace),
                    Heuristic::Miss(stacktrace) => stacktrace,
                }
            };
        }

        let result = Self::check_last_submessage(trace, stacktrace, last_submessage_data)?;
        let stacktrace = return_if_hit!(result);

        let result = Self::check_failed_last_call(trace, stacktrace)?;
        let stacktrace = return_if_hit!(result);

        let result = Self::check_last_instruction(
            trace,
            stacktrace,
            function_jumpdests,
            jumped_into_function,
        )?;
        let stacktrace = return_if_hit!(result);

        let result = Self::check_non_contract_called(trace, stacktrace)?;
        let stacktrace = return_if_hit!(result);

        let result = Self::check_solidity_0_6_3_unmapped_revert(trace, stacktrace)?;
        let stacktrace = return_if_hit!(result);

        if let Some(result) = Self::check_contract_too_large(trace)? {
            return Ok(result);
        }

        let stacktrace = Self::other_execution_error_stacktrace(trace, stacktrace)?;
        Ok(stacktrace)
    }

    pub fn filter_redundant_frames(
        stacktrace: SolidityStackTrace,
    ) -> napi::Result<SolidityStackTrace> {
        // To work around the borrow checker, we'll collect the indices of the frames we
        // want to keep. We can't clone the frames, because some of them contain
        // non-Clone `ClassInstance`s`
        let retained_indices: HashSet<_> = stacktrace
            .iter()
            .enumerate()
            .filter(|(idx, frame)| {
                let next_frame = stacktrace.get(idx + 1);
                let next_next_frame = stacktrace.get(idx + 2);

                let Some(next_frame) = next_frame else {
                    return true;
                };

                // we can only filter frames if we know their sourceReference
                // and the one from the next frame
                let (Some(frame_source), Some(next_frame_source)) =
                    (frame.source_reference(), next_frame.source_reference())
                else {
                    return true;
                };

                // look TWO frames ahead to determine if this is a specific occurrence of
                // a redundant CALLSTACK_ENTRY frame observed when using Solidity 0.8.5:
                match (&frame, next_next_frame) {
                    (
                        Either24::A(CallstackEntryStackTraceEntry {
                            source_reference, ..
                        }),
                        Some(Either24::N(ReturndataSizeErrorStackTraceEntry {
                            source_reference: next_next_source_reference,
                            ..
                        })),
                    ) if source_reference.range == next_next_source_reference.range
                        && source_reference.line == next_next_source_reference.line =>
                    {
                        return false;
                    }
                    _ => {}
                }

                if frame_source.function.as_deref() == Some("constructor")
                    && next_frame_source.function.as_deref() != Some("constructor")
                {
                    return true;
                }

                // this is probably a recursive call
                if *idx > 0
                    && frame.type_() == next_frame.type_()
                    && frame_source.range == next_frame_source.range
                    && frame_source.line == next_frame_source.line
                {
                    return true;
                }

                if frame_source.range[0] <= next_frame_source.range[0]
                    && frame_source.range[1] >= next_frame_source.range[1]
                {
                    return false;
                }

                true
            })
            .map(|(idx, _)| idx)
            .collect();

        Ok(stacktrace
            .into_iter()
            .enumerate()
            .filter(|(idx, _)| retained_indices.contains(idx))
            .map(|(_, frame)| frame)
            .collect())
    }

    // Heuristics

    fn check_contract_too_large(
        trace: Either<&CallMessageTrace, &CreateMessageTrace>,
    ) -> napi::Result<Option<SolidityStackTrace>> {
        Ok(match trace {
            Either::B(create @ CreateMessageTrace { .. })
                if create.exit.kind() == ExitCode::CODESIZE_EXCEEDS_MAXIMUM =>
            {
                Some(vec![ContractTooLargeErrorStackTraceEntry {
                    type_: StackTraceEntryTypeConst,
                    source_reference: Some(Self::get_constructor_start_source_reference(create)?),
                }
                .into()])
            }

            _ => None,
        })
    }
    /// Check if the last call/create that was done failed.
    fn check_failed_last_call(
        trace: Either<&CallMessageTrace, &CreateMessageTrace>,
        stacktrace: SolidityStackTrace,
    ) -> napi::Result<Heuristic> {
        let (bytecode, steps) = match &trace {
            Either::A(call) => (&call.bytecode, &call.steps),
            Either::B(create) => (&create.bytecode, &create.steps),
        };

        let bytecode = bytecode.as_ref().expect("JS code asserts");

        if steps.is_empty() {
            return Ok(Heuristic::Miss(stacktrace));
        }

        for step_index in (0..steps.len() - 1).rev() {
            let (step, next_step) = match &steps[step_index..][..2] {
                &[Either4::A(ref step), ref next_step] => (step, next_step),
                _ => return Ok(Heuristic::Miss(stacktrace)),
            };

            let inst = bytecode.get_instruction(step.pc)?;

            if let (OpCode::CALL | OpCode::CREATE, Either4::A(EvmStep { .. })) =
                (inst.opcode, next_step)
            {
                if Self::is_call_failed_error(trace, step_index as u32, inst)? {
                    let mut inferred_stacktrace = stacktrace.clone();
                    inferred_stacktrace.push(
                        Self::call_instruction_to_call_failed_to_execute_stack_trace_entry(
                            bytecode, inst,
                        )?
                        .into(),
                    );

                    return Ok(Heuristic::Hit(Self::fix_initial_modifier(
                        trace,
                        inferred_stacktrace,
                    )?));
                }
            }
        }

        Ok(Heuristic::Miss(stacktrace))
    }

    /// Check if the trace reverted with a panic error.
    fn check_panic(
        trace: Either<&CallMessageTrace, &CreateMessageTrace>,
        mut stacktrace: SolidityStackTrace,
        last_instruction: &Instruction,
    ) -> napi::Result<Heuristic> {
        let return_data = ReturnData::new(
            match &trace {
                Either::A(call) => &call.return_data,
                Either::B(create) => &create.return_data,
            }
            .clone(),
        );

        if !return_data.is_panic_return_data() {
            return Ok(Heuristic::Miss(stacktrace));
        }

        // If the last frame is an internal function, it means that the trace
        // jumped there to return the panic. If that's the case, we remove that
        // frame.
        if let Some(Either24::W(InternalFunctionCallStackEntry { .. })) = stacktrace.last() {
            stacktrace.pop();
        }

        // if the error comes from a call to a zero-initialized function,
        // we remove the last frame, which represents the call, to avoid
        // having duplicated frames
        let error_code = return_data.decode_panic()?;
        let (panic_error_code, lossless) = error_code.get_i64();
        if let (0x51, false) = (panic_error_code, lossless) {
            stacktrace.pop();
        }

        stacktrace.push(
            Self::instruction_within_function_to_panic_stack_trace_entry(
                trace,
                last_instruction,
                error_code,
            )?
            .into(),
        );

        Self::fix_initial_modifier(trace, stacktrace).map(Heuristic::Hit)
    }

    fn check_custom_errors(
        trace: Either<&CallMessageTrace, &CreateMessageTrace>,
        stacktrace: SolidityStackTrace,
        last_instruction: &Instruction,
    ) -> napi::Result<Heuristic> {
        let (bytecode, return_data) = match &trace {
            Either::A(call) => (&call.bytecode, &call.return_data),
            Either::B(create) => (&create.bytecode, &create.return_data),
        };

        let bytecode = bytecode.as_ref().expect("JS code asserts");
        let contract = bytecode.contract.borrow();

        let return_data = ReturnData::new(return_data.clone());

        if return_data.is_empty() || return_data.is_error_return_data() {
            // if there is no return data, or if it's a Error(string),
            // then it can't be a custom error
            return Ok(Heuristic::Miss(stacktrace));
        }

        let raw_return_data = hex::encode(&*return_data.value);
        let mut error_message = format!(
            "reverted with an unrecognized custom error (return data: 0x{raw_return_data})",
        );

        for custom_error in &contract.custom_errors {
            if return_data.matches_selector(custom_error.selector) {
                // if the return data matches a custom error in the called contract,
                // we format the message using the returnData and the custom error instance
                let decoded = custom_error
                    .decode_error_data(&return_data.value)
                    .map_err(|e| {
                        napi::Error::from_reason(format!("Error decoding custom error: {e}"))
                    })?;

                let params = decoded
                    .body
                    .iter()
                    .map(format_dyn_sol_value)
                    .collect::<Vec<_>>();

                error_message = format!(
                    "reverted with custom error '{name}({params})'",
                    name = custom_error.name,
                    params = params.join(", ")
                );

                break;
            }
        }

        let mut stacktrace = stacktrace;
        stacktrace.push(
            Self::instruction_within_function_to_custom_error_stack_trace_entry(
                trace,
                last_instruction,
                error_message,
            )?
            .into(),
        );

        Self::fix_initial_modifier(trace, stacktrace).map(Heuristic::Hit)
    }

    fn check_solidity_0_6_3_unmapped_revert(
        trace: Either<&CallMessageTrace, &CreateMessageTrace>,
        mut stacktrace: SolidityStackTrace,
    ) -> napi::Result<Heuristic> {
        if Self::solidity_0_6_3_maybe_unmapped_revert(trace)? {
            let revert_frame =
                Self::solidity_0_6_3_get_frame_for_unmapped_revert_within_function(trace)?;

            if let Some(revert_frame) = revert_frame {
                stacktrace.push(revert_frame.into());

                return Ok(Heuristic::Hit(stacktrace));
            }

            return Ok(Heuristic::Hit(stacktrace));
        }

        Ok(Heuristic::Miss(stacktrace))
    }

    fn check_non_contract_called(
        trace: Either<&CallMessageTrace, &CreateMessageTrace>,
        mut stacktrace: SolidityStackTrace,
    ) -> napi::Result<Heuristic> {
        if Self::is_called_non_contract_account_error(trace)? {
            let source_reference = Self::get_last_source_reference(trace)?;

            // We are sure this is not undefined because there was at least a call
            // instruction
            let source_reference =
                source_reference.expect("Expected source reference to be defined");

            let non_contract_called_frame = NonContractAccountCalledErrorStackTraceEntry {
                type_: StackTraceEntryTypeConst,
                source_reference,
            };

            stacktrace.push(non_contract_called_frame.into());

            Ok(Heuristic::Hit(stacktrace))
        } else {
            Ok(Heuristic::Miss(stacktrace))
        }
    }

    /// Check if the last submessage can be used to generate the stack trace.
    fn check_last_submessage(
        trace: Either<&CallMessageTrace, &CreateMessageTrace>,
        stacktrace: SolidityStackTrace,
        last_submessage_data: Option<SubmessageDataRef<'_>>,
    ) -> napi::Result<Heuristic> {
        let (bytecode, steps) = match &trace {
            Either::A(call) => (&call.bytecode, &call.steps),
            Either::B(create) => (&create.bytecode, &create.steps),
        };

        let bytecode = bytecode.as_ref().expect("JS code asserts");

        let Some(last_submessage_data) = last_submessage_data else {
            return Ok(Heuristic::Miss(stacktrace));
        };

        let mut inferred_stacktrace = Cow::from(&stacktrace);

        // get the instruction before the submessage and add it to the stack trace
        let call_step = match steps.get(last_submessage_data.step_index as usize - 1) {
            Some(Either4::A(call_step)) => call_step,
            _ => panic!("This should not happen: MessageTrace should be preceded by a EVM step"),
        };

        let call_inst = bytecode.get_instruction(call_step.pc)?;
        let call_stack_frame = instruction_to_callstack_stack_trace_entry(bytecode, call_inst)?;

        let (call_stack_frame_source_reference, call_stack_frame) = match call_stack_frame {
            Either::A(frame) => (frame.source_reference.clone(), frame.into()),
            Either::B(frame) => (frame.source_reference.clone(), frame.into()),
        };

        let last_message_failed = match last_submessage_data.message_trace {
            Either3::A(precompile) => precompile.exit.is_error(),
            Either3::B(call) => call.exit.is_error(),
            Either3::C(create) => create.exit.is_error(),
        };
        if last_message_failed {
            // add the call/create that generated the message to the stack trace
            let inferred_stacktrace = inferred_stacktrace.to_mut();
            inferred_stacktrace.push(call_stack_frame);

            if Self::is_subtrace_error_propagated(trace, last_submessage_data.step_index)?
                || Self::is_proxy_error_propagated(trace, last_submessage_data.step_index)?
            {
                inferred_stacktrace.extend(last_submessage_data.stacktrace);

                if Self::is_contract_call_run_out_of_gas_error(
                    trace,
                    last_submessage_data.step_index,
                )? {
                    let last_frame = match inferred_stacktrace.pop() {
                        Some(frame) => frame,
                        _ => panic!("Expected inferred stack trace to have at least one frame"),
                    };

                    inferred_stacktrace.push(
                        ContractCallRunOutOfGasError {
                            type_: StackTraceEntryTypeConst,
                            source_reference: last_frame.source_reference().cloned(),
                        }
                        .into(),
                    );
                }

                return Self::fix_initial_modifier(trace, inferred_stacktrace.to_owned())
                    .map(Heuristic::Hit);
            }
        } else {
            let is_return_data_size_error =
                Self::fails_right_after_call(trace, last_submessage_data.step_index)?;
            if is_return_data_size_error {
                inferred_stacktrace.to_mut().push(
                    ReturndataSizeErrorStackTraceEntry {
                        type_: StackTraceEntryTypeConst,
                        source_reference: call_stack_frame_source_reference,
                    }
                    .into(),
                );

                return Self::fix_initial_modifier(trace, inferred_stacktrace.into_owned())
                    .map(Heuristic::Hit);
            }
        }

        Ok(Heuristic::Miss(stacktrace))
    }

    /// Check if the execution stopped with a revert or an invalid opcode.
    fn check_revert_or_invalid_opcode(
        trace: Either<&CallMessageTrace, &CreateMessageTrace>,
        stacktrace: SolidityStackTrace,
        last_instruction: &Instruction,
        function_jumpdests: &[&Instruction],
        jumped_into_function: bool,
    ) -> napi::Result<Heuristic> {
        match last_instruction.opcode {
            OpCode::REVERT | OpCode::INVALID => {}
            _ => return Ok(Heuristic::Miss(stacktrace)),
        }

        let (bytecode, return_data) = match &trace {
            Either::A(call) => (&call.bytecode, &call.return_data),
            Either::B(create) => (&create.bytecode, &create.return_data),
        };
        let bytecode = bytecode.as_ref().expect("JS code asserts");

        let mut inferred_stacktrace = stacktrace.clone();

        if let Some(location) = &last_instruction.location {
            if jumped_into_function || matches!(trace, Either::B(CreateMessageTrace { .. })) {
                // There should always be a function here, but that's not the case with
                // optimizations.
                //
                // If this is a create trace, we already checked args and nonpayable failures
                // before calling this function.
                //
                // If it's a call trace, we already jumped into a function. But optimizations
                // can happen.
                let failing_function = location.get_containing_function();

                // If the failure is in a modifier we add an entry with the function/constructor
                match failing_function {
                    Some(func) if func.r#type == ContractFunctionType::Modifier => {
                        let frame =
                            Self::get_entry_before_failure_in_modifier(trace, function_jumpdests)?;

                        inferred_stacktrace.push(match frame {
                            Either::A(frame) => frame.into(),
                            Either::B(frame) => frame.into(),
                        });
                    }
                    _ => {}
                }
            }
        }

        let panic_stacktrace = Self::check_panic(trace, inferred_stacktrace, last_instruction)?;
        let inferred_stacktrace = match panic_stacktrace {
            hit @ Heuristic::Hit(..) => return Ok(hit),
            Heuristic::Miss(stacktrace) => stacktrace,
        };

        let custom_error_stacktrace =
            Self::check_custom_errors(trace, inferred_stacktrace, last_instruction)?;
        let mut inferred_stacktrace = match custom_error_stacktrace {
            hit @ Heuristic::Hit(..) => return Ok(hit),
            Heuristic::Miss(stacktrace) => stacktrace,
        };

        if let Some(location) = &last_instruction.location {
            if jumped_into_function || matches!(trace, Either::B(CreateMessageTrace { .. })) {
                let failing_function = location.get_containing_function();

                if failing_function.is_some() {
                    let frame = Self::instruction_within_function_to_revert_stack_trace_entry(
                        trace,
                        last_instruction,
                    )?;

                    inferred_stacktrace.push(frame.into());
                } else {
                    let is_invalid_opcode_error = last_instruction.opcode == OpCode::INVALID;

                    match &trace {
                        Either::A(CallMessageTrace { calldata, .. }) => {
                            let contract = bytecode.contract.borrow();

                            // This is here because of the optimizations
                            let function_from_selector = contract.get_function_from_selector(
                                calldata.get(..4).unwrap_or(&calldata[..]),
                            );

                            // in general this shouldn't happen, but it does when viaIR is enabled,
                            // "optimizerSteps": "u" is used, and the called function is fallback or
                            // receive
                            let Some(function) = function_from_selector else {
                                return Ok(Heuristic::Miss(inferred_stacktrace));
                            };

                            let frame = RevertErrorStackTraceEntry {
                                type_: StackTraceEntryTypeConst,
                                source_reference: Self::get_function_start_source_reference(
                                    trace, function,
                                )?,
                                return_data: return_data.clone(),
                                is_invalid_opcode_error,
                            };

                            inferred_stacktrace.push(frame.into());
                        }
                        Either::B(trace @ CreateMessageTrace { .. }) => {
                            // This is here because of the optimizations
                            let frame = RevertErrorStackTraceEntry {
                                type_: StackTraceEntryTypeConst,
                                source_reference: Self::get_constructor_start_source_reference(
                                    trace,
                                )?,
                                return_data: return_data.clone(),
                                is_invalid_opcode_error,
                            };

                            inferred_stacktrace.push(frame.into());
                        }
                    }
                }

                return Self::fix_initial_modifier(trace, inferred_stacktrace).map(Heuristic::Hit);
            }
        }

        // If the revert instruction is not mapped but there is return data,
        // we add the frame anyway, sith the best sourceReference we can get
        if last_instruction.location.is_none() && !return_data.is_empty() {
            let revert_frame = RevertErrorStackTraceEntry {
                type_: StackTraceEntryTypeConst,
                source_reference: Self::get_contract_start_without_function_source_reference(
                    trace,
                )?,
                return_data: return_data.clone(),
                is_invalid_opcode_error: last_instruction.opcode == OpCode::INVALID,
            };

            inferred_stacktrace.push(revert_frame.into());

            return Self::fix_initial_modifier(trace, inferred_stacktrace).map(Heuristic::Hit);
        }

        Ok(Heuristic::Miss(stacktrace))
    }

    fn check_last_instruction(
        trace: Either<&CallMessageTrace, &CreateMessageTrace>,
        stacktrace: SolidityStackTrace,
        function_jumpdests: &[&Instruction],
        jumped_into_function: bool,
    ) -> napi::Result<Heuristic> {
        let (bytecode, steps) = match &trace {
            Either::A(call) => (&call.bytecode, &call.steps),
            Either::B(create) => (&create.bytecode, &create.steps),
        };
        let bytecode = bytecode.as_ref().expect("JS code asserts");

        if steps.is_empty() {
            return Ok(Heuristic::Miss(stacktrace));
        }

        let last_step = match steps.last() {
            Some(Either4::A(step)) => step,
            _ => panic!("This should not happen: MessageTrace ends with a subtrace"),
        };

        let last_instruction = bytecode.get_instruction(last_step.pc)?;

        let revert_or_invalid_stacktrace = Self::check_revert_or_invalid_opcode(
            trace,
            stacktrace,
            last_instruction,
            function_jumpdests,
            jumped_into_function,
        )?;
        let stacktrace = match revert_or_invalid_stacktrace {
            hit @ Heuristic::Hit(..) => return Ok(hit),
            Heuristic::Miss(stacktrace) => stacktrace,
        };

        let (Either::A(trace @ CallMessageTrace { ref calldata, .. }), false) =
            (&trace, jumped_into_function)
        else {
            return Ok(Heuristic::Miss(stacktrace));
        };

        if Self::has_failed_inside_the_fallback_function(trace)?
            || Self::has_failed_inside_the_receive_function(trace)?
        {
            let frame = Self::instruction_within_function_to_revert_stack_trace_entry(
                Either::A(trace),
                last_instruction,
            )?;

            return Ok(Heuristic::Hit(vec![frame.into()]));
        }

        // Sometimes we do fail inside of a function but there's no jump into
        if let Some(location) = &last_instruction.location {
            let failing_function = location.get_containing_function();

            if let Some(failing_function) = failing_function {
                let frame = RevertErrorStackTraceEntry {
                    type_: StackTraceEntryTypeConst,
                    source_reference: Self::get_function_start_source_reference(
                        Either::A(trace),
                        &failing_function,
                    )?,
                    return_data: trace.return_data.clone(),
                    is_invalid_opcode_error: last_instruction.opcode == OpCode::INVALID,
                };

                return Ok(Heuristic::Hit(vec![frame.into()]));
            }
        }

        let contract = bytecode.contract.borrow();

        let selector = calldata.get(..4).unwrap_or(&calldata[..]);
        let calldata = &calldata.get(4..).unwrap_or(&[]);

        let called_function = contract.get_function_from_selector(selector);

        if let Some(called_function) = called_function {
            let abi = alloy_json_abi::Function::try_from(&**called_function).map_err(|e| {
                napi::Error::from_reason(format!("Error converting to alloy ABI: {e}"))
            })?;

            let is_valid_calldata = match &called_function.param_types {
                Some(_) => abi.abi_decode_input(calldata, true).is_ok(),
                // if we don't know the param types, we just assume that the call is valid
                None => true,
            };

            if !is_valid_calldata {
                let frame = InvalidParamsErrorStackTraceEntry {
                    type_: StackTraceEntryTypeConst,
                    source_reference: Self::get_function_start_source_reference(
                        Either::A(trace),
                        called_function,
                    )?,
                };

                return Ok(Heuristic::Hit(vec![frame.into()]));
            }
        }

        if Self::solidity_0_6_3_maybe_unmapped_revert(Either::A(trace))? {
            let revert_frame =
                Self::solidity_0_6_3_get_frame_for_unmapped_revert_before_function(trace)?;

            if let Some(revert_frame) = revert_frame {
                return Ok(Heuristic::Hit(vec![revert_frame.into()]));
            }
        }

        let frame = Self::get_other_error_before_called_function_stack_trace_entry(trace)?;

        Ok(Heuristic::Hit(vec![frame.into()]))
    }

    // Helpers

    fn fix_initial_modifier(
        trace: Either<&CallMessageTrace, &CreateMessageTrace>,
        mut stacktrace: SolidityStackTrace,
    ) -> napi::Result<SolidityStackTrace> {
        if let Some(Either24::A(CallstackEntryStackTraceEntry {
            function_type: ContractFunctionTypeNapi::MODIFIER,
            ..
        })) = stacktrace.first()
        {
            let entry_before_initial_modifier =
                Self::get_entry_before_initial_modifier_callstack_entry(trace)?;

            stacktrace.insert(0, entry_before_initial_modifier);
        }

        Ok(stacktrace)
    }

    fn get_entry_before_initial_modifier_callstack_entry(
        trace: Either<&CallMessageTrace, &CreateMessageTrace>,
    ) -> napi::Result<SolidityStackTraceEntry> {
        let trace = match trace {
            Either::B(create) => {
                return Ok(CallstackEntryStackTraceEntry {
                    type_: StackTraceEntryTypeConst,
                    source_reference: Self::get_constructor_start_source_reference(create)?,
                    function_type: ContractFunctionType::Constructor.into(),
                }
                .into())
            }
            Either::A(call) => call,
        };

        let bytecode = trace.bytecode.as_ref().expect("JS code asserts");
        let contract = bytecode.contract.borrow();

        let called_function = contract
            .get_function_from_selector(trace.calldata.get(..4).unwrap_or(&trace.calldata[..]));

        let source_reference = match called_function {
            Some(called_function) => {
                Self::get_function_start_source_reference(Either::A(trace), called_function)?
            }
            None => Self::get_fallback_start_source_reference(trace)?,
        };

        let function_type = match called_function {
            Some(_) => ContractFunctionType::Function,
            None => ContractFunctionType::Fallback,
        };

        Ok(CallstackEntryStackTraceEntry {
            type_: StackTraceEntryTypeConst,
            source_reference,
            function_type: function_type.into(),
        }
        .into())
    }

    fn call_instruction_to_call_failed_to_execute_stack_trace_entry(
        bytecode: &Bytecode,
        call_inst: &Instruction,
    ) -> napi::Result<CallFailedErrorStackTraceEntry> {
        let location = call_inst.location.as_deref();

        let source_reference = source_location_to_source_reference(bytecode, location)?;
        let source_reference = source_reference.expect("Expected source reference to be defined");

        // Calls only happen within functions
        Ok(CallFailedErrorStackTraceEntry {
            type_: StackTraceEntryTypeConst,
            source_reference,
        })
    }

    fn get_entry_before_failure_in_modifier(
        trace: Either<&CallMessageTrace, &CreateMessageTrace>,
        function_jumpdests: &[&Instruction],
    ) -> napi::Result<Either<CallstackEntryStackTraceEntry, InternalFunctionCallStackEntry>> {
        let bytecode = match &trace {
            Either::A(call) => &call.bytecode,
            Either::B(create) => &create.bytecode,
        };
        let bytecode = bytecode.as_ref().expect("JS code asserts");

        // If there's a jumpdest, this modifier belongs to the last function that it
        // represents
        if let Some(last_jumpdest) = function_jumpdests.last() {
            let entry = instruction_to_callstack_stack_trace_entry(bytecode, last_jumpdest)?;

            return Ok(entry);
        }

        // This function is only called after we jumped into the initial function in
        // call traces, so there should always be at least a function jumpdest.
        let trace = match trace {
            Either::A(_call) => return Err(
              napi::Error::new(
                napi::Status::GenericFailure,
                "This shouldn't happen: a call trace has no functionJumpdest but has already jumped into a function"
              )),
            Either::B(create) => create,
        };

        // If there's no jump dest, we point to the constructor.
        Ok(Either::A(CallstackEntryStackTraceEntry {
            type_: StackTraceEntryTypeConst,
            source_reference: Self::get_constructor_start_source_reference(trace)?,
            function_type: ContractFunctionType::Constructor.into(),
        }))
    }

    fn fails_right_after_call(
        trace: Either<&CallMessageTrace, &CreateMessageTrace>,
        call_subtrace_step_index: u32,
    ) -> napi::Result<bool> {
        let (bytecode, steps) = match &trace {
            Either::A(call) => (&call.bytecode, &call.steps),
            Either::B(create) => (&create.bytecode, &create.steps),
        };

        let bytecode = bytecode.as_ref().expect("JS code asserts");

        let Some(Either4::A(last_step)) = steps.last() else {
            return Ok(false);
        };

        let last_inst = bytecode.get_instruction(last_step.pc)?;
        if last_inst.opcode != OpCode::REVERT {
            return Ok(false);
        }

        let call_opcode_step = steps.get(call_subtrace_step_index as usize - 1);
        let call_opcode_step = match call_opcode_step {
            Some(Either4::A(step)) => step,
            _ => panic!("JS code asserts this is always an EvmStep"),
        };
        let call_inst = bytecode.get_instruction(call_opcode_step.pc)?;

        // Calls are always made from within functions
        let call_inst_location = call_inst
            .location
            .as_ref()
            .expect("Expected call instruction location to be defined");

        Self::is_last_location(trace, call_subtrace_step_index + 1, call_inst_location)
    }

    fn is_call_failed_error(
        trace: Either<&CallMessageTrace, &CreateMessageTrace>,
        inst_index: u32,
        call_instruction: &Instruction,
    ) -> napi::Result<bool> {
        let call_location = match &call_instruction.location {
            Some(location) => location,
            None => panic!("Expected call location to be defined"),
        };

        Self::is_last_location(trace, inst_index, call_location)
    }

    fn is_last_location(
        trace: Either<&CallMessageTrace, &CreateMessageTrace>,
        from_step: u32,
        location: &SourceLocation,
    ) -> napi::Result<bool> {
        let (bytecode, steps) = match &trace {
            Either::A(call) => (&call.bytecode, &call.steps),
            Either::B(create) => (&create.bytecode, &create.steps),
        };

        let bytecode = bytecode.as_ref().expect("JS code asserts");

        for step in steps.iter().skip(from_step as usize) {
            let step = match step {
                Either4::A(step) => step,
                _ => return Ok(false),
            };

            let step_inst = bytecode.get_instruction(step.pc)?;

            if let Some(step_inst_location) = &step_inst.location {
                if **step_inst_location != *location {
                    return Ok(false);
                }
            }
        }

        Ok(true)
    }

    fn is_subtrace_error_propagated(
        trace: Either<&CallMessageTrace, &CreateMessageTrace>,
        call_subtrace_step_index: u32,
    ) -> napi::Result<bool> {
        let (return_data, exit, steps) = match &trace {
            Either::A(call) => (&call.return_data, call.exit.kind(), &call.steps),
            Either::B(create) => (&create.return_data, create.exit.kind(), &create.steps),
        };

        let (call_return_data, call_exit) = match steps.get(call_subtrace_step_index as usize) {
            None | Some(Either4::A(_)) => panic!("Expected call to be a message trace"),
            Some(Either4::B(precompile)) => (&precompile.return_data, precompile.exit.kind()),
            Some(Either4::C(call)) => (&call.return_data, call.exit.kind()),
            Some(Either4::D(create)) => (&create.return_data, create.exit.kind()),
        };

        if return_data.as_ref() != call_return_data.as_ref() {
            return Ok(false);
        }

        if exit == ExitCode::OUT_OF_GAS && call_exit == ExitCode::OUT_OF_GAS {
            return Ok(true);
        }

        // If the return data is not empty, and it's still the same, we assume it
        // is being propagated
        if return_data.len() > 0 {
            return Ok(true);
        }

        Self::fails_right_after_call(trace, call_subtrace_step_index)
    }

    fn is_contract_call_run_out_of_gas_error(
        trace: Either<&CallMessageTrace, &CreateMessageTrace>,
        call_step_index: u32,
    ) -> napi::Result<bool> {
        let (steps, return_data, exit_code) = match &trace {
            Either::A(call) => (&call.steps, &call.return_data, call.exit.kind()),
            Either::B(create) => (&create.steps, &create.return_data, create.exit.kind()),
        };

        if return_data.len() > 0 {
            return Ok(false);
        }

        if exit_code != ExitCode::REVERT {
            return Ok(false);
        }

        let call_exit = match steps.get(call_step_index as usize) {
            None | Some(Either4::A(_)) => panic!("Expected call to be a message trace"),
            Some(Either4::B(precompile)) => precompile.exit.kind(),
            Some(Either4::C(call)) => call.exit.kind(),
            Some(Either4::D(create)) => create.exit.kind(),
        };

        if call_exit != ExitCode::OUT_OF_GAS {
            return Ok(false);
        }

        Self::fails_right_after_call(trace, call_step_index)
    }

    fn is_proxy_error_propagated(
        trace: Either<&CallMessageTrace, &CreateMessageTrace>,
        call_subtrace_step_index: u32,
    ) -> napi::Result<bool> {
        let trace = match &trace {
            Either::A(call) => call,
            Either::B(_) => return Ok(false),
        };

        let bytecode = trace.bytecode.as_ref().expect("JS code asserts");

        let call_step = match trace.steps.get(call_subtrace_step_index as usize - 1) {
            Some(Either4::A(step)) => step,
            _ => return Ok(false),
        };

        let call_inst = bytecode.get_instruction(call_step.pc)?;

        if call_inst.opcode != OpCode::DELEGATECALL {
            return Ok(false);
        }

        let subtrace = match trace.steps.get(call_subtrace_step_index as usize) {
            None | Some(Either4::A(_) | Either4::B(_)) => return Ok(false),
            Some(Either4::C(call)) => Either::A(call),
            Some(Either4::D(create)) => Either::B(create),
        };

        let (subtrace_bytecode, subtrace_return_data) = match &subtrace {
            Either::A(call) => (&call.bytecode, &call.return_data),
            Either::B(create) => (&create.bytecode, &create.return_data),
        };
        let subtrace_bytecode = match subtrace_bytecode {
            Some(bytecode) => bytecode,
            // If we can't recognize the implementation we'd better don't consider it as such
            None => return Ok(false),
        };

        if subtrace_bytecode.contract.borrow().r#type == ContractKind::Library {
            return Ok(false);
        }

        if trace.return_data.as_ref() != subtrace_return_data.as_ref() {
            return Ok(false);
        }

        for step in trace
            .steps
            .iter()
            .skip(call_subtrace_step_index as usize + 1)
        {
            let step = match step {
                Either4::A(step) => step,
                _ => return Ok(false),
            };

            let inst = bytecode.get_instruction(step.pc)?;

            // All the remaining locations should be valid, as they are part of the inline
            // asm
            if inst.location.is_none() {
                return Ok(false);
            }

            if matches!(
                inst.jump_type,
                JumpType::IntoFunction | JumpType::OutofFunction
            ) {
                return Ok(false);
            }
        }

        let last_step = match trace.steps.last() {
            Some(Either4::A(step)) => step,
            _ => panic!("Expected last step to be an EvmStep"),
        };
        let last_inst = bytecode.get_instruction(last_step.pc)?;

        Ok(last_inst.opcode == OpCode::REVERT)
    }

    fn other_execution_error_stacktrace(
        trace: Either<&CallMessageTrace, &CreateMessageTrace>,
        mut stacktrace: SolidityStackTrace,
    ) -> napi::Result<SolidityStackTrace> {
        let other_execution_error_frame = OtherExecutionErrorStackTraceEntry {
            type_: StackTraceEntryTypeConst,
            source_reference: Self::get_last_source_reference(trace)?,
        };

        stacktrace.push(other_execution_error_frame.into());
        Ok(stacktrace)
    }

    fn is_direct_library_call(trace: &CallMessageTrace) -> napi::Result<bool> {
        let contract = &trace.bytecode.as_ref().expect("JS code asserts").contract;
        let contract = contract.borrow();

        Ok(trace.depth == 0 && contract.r#type == ContractKind::Library)
    }

    fn get_direct_library_call_error_stack_trace(
        trace: &CallMessageTrace,
    ) -> napi::Result<SolidityStackTrace> {
        let contract = &trace.bytecode.as_ref().expect("JS code asserts").contract;
        let contract = contract.borrow();

        let func = contract
            .get_function_from_selector(trace.calldata.get(..4).unwrap_or(&trace.calldata[..]));

        let source_reference = match func {
            Some(func) => Self::get_function_start_source_reference(Either::A(trace), func)?,
            None => Self::get_contract_start_without_function_source_reference(Either::A(trace))?,
        };

        Ok(vec![DirectLibraryCallErrorStackTraceEntry {
            type_: StackTraceEntryTypeConst,
            source_reference,
        }
        .into()])
    }

    fn get_function_start_source_reference(
        trace: Either<&CallMessageTrace, &CreateMessageTrace>,
        func: &ContractFunction,
    ) -> napi::Result<SourceReference> {
        let bytecode = match &trace {
            Either::A(create) => &create.bytecode,
            Either::B(call) => &call.bytecode,
        };

        let contract = &bytecode.as_ref().expect("JS code asserts").contract;
        let contract = contract.borrow();

        let file = func.location.file();
        let file = file.borrow();

        let location = &func.location;

        Ok(SourceReference {
            source_name: file.source_name.clone(),
            source_content: file.content.clone(),
            contract: Some(contract.name.clone()),

            function: Some(func.name.clone()),
            line: location.get_starting_line_number(),
            range: [location.offset, location.offset + location.length].to_vec(),
        })
    }

    fn is_missing_function_and_fallback_error(
        trace: &CallMessageTrace,
        called_function: Option<&ContractFunction>,
    ) -> napi::Result<bool> {
        // This error doesn't return data
        if trace.return_data.len() > 0 {
            return Ok(false);
        }

        // the called function exists in the contract
        if called_function.is_some() {
            return Ok(false);
        }

        let bytecode = trace
            .bytecode
            .as_ref()
            .expect("The TS code type-checks this to always have bytecode");
        let contract = bytecode.contract.borrow();

        // there's a receive function and no calldata
        if trace.calldata.len() == 0 && contract.receive.is_some() {
            return Ok(false);
        }

        Ok(contract.fallback.is_none())
    }

    fn empty_calldata_and_no_receive(trace: &CallMessageTrace) -> napi::Result<bool> {
        let bytecode = trace
            .bytecode
            .as_ref()
            .expect("The TS code type-checks this to always have bytecode");
        let contract = bytecode.contract.borrow();

        let version =
            Version::parse(&bytecode.compiler_version).expect("Failed to parse SemVer version");

        // this only makes sense when receive functions are available
        if version < FIRST_SOLC_VERSION_RECEIVE_FUNCTION {
            return Ok(false);
        }

        Ok(trace.calldata.is_empty() && contract.receive.is_none())
    }

    fn is_fallback_not_payable_error(
        trace: &CallMessageTrace,
        called_function: Option<&ContractFunction>,
    ) -> napi::Result<bool> {
        // This error doesn't return data
        if !trace.return_data.is_empty() {
            return Ok(false);
        }

        let (neg, value, _) = trace.value.get_u64();
        if neg || value == 0 {
            return Ok(false);
        }

        // the called function exists in the contract
        if called_function.is_some() {
            return Ok(false);
        }

        let bytecode = trace.bytecode.as_ref().expect("JS code asserts");
        let contract = bytecode.contract.borrow();

        match &contract.fallback {
            Some(fallback) => Ok(fallback.is_payable != Some(true)),
            None => Ok(false),
        }
    }

    fn get_fallback_start_source_reference(
        trace: &CallMessageTrace,
    ) -> napi::Result<SourceReference> {
        let bytecode = trace.bytecode.as_ref().expect("JS code asserts");
        let contract = bytecode.contract.borrow();

        let func = match &contract.fallback {
          Some(func) => func,
          None => panic!("This shouldn't happen: trying to get fallback source reference from a contract without fallback"),
        };

        let location = &func.location;
        let file = location.file();
        let file = file.borrow();

        Ok(SourceReference {
            source_name: file.source_name.clone(),
            source_content: file.content.clone(),
            contract: Some(contract.name.clone()),
            function: Some(FALLBACK_FUNCTION_NAME.to_string()),
            line: location.get_starting_line_number(),
            range: [location.offset, location.offset + location.length].to_vec(),
        })
    }

    fn is_constructor_not_payable_error(trace: &CreateMessageTrace) -> napi::Result<bool> {
        // This error doesn't return data
        if !trace.return_data.is_empty() {
            return Ok(false);
        }

        let bytecode = trace.bytecode.as_ref().expect("JS code asserts");
        let contract = bytecode.contract.borrow();

        // This function is only matters with contracts that have constructors defined.
        // The ones that don't are abstract contracts, or their constructor
        // doesn't take any argument.
        let constructor = match &contract.constructor {
            Some(constructor) => constructor,
            None => return Ok(false),
        };

        let (neg, value, _) = trace.value.get_u64();
        if neg || value == 0 {
            return Ok(false);
        }

        Ok(constructor.is_payable != Some(true))
    }

    /// Returns a source reference pointing to the constructor if it exists, or
    /// to the contract otherwise.
    fn get_constructor_start_source_reference(
        trace: &CreateMessageTrace,
    ) -> napi::Result<SourceReference> {
        let bytecode = trace.bytecode.as_ref().expect("JS code asserts");
        let contract = bytecode.contract.borrow();
        let contract_location = &contract.location;

        let line = match &contract.constructor {
            Some(constructor) => constructor.location.get_starting_line_number(),
            None => contract_location.get_starting_line_number(),
        };

        let file = contract_location.file();
        let file = file.borrow();

        Ok(SourceReference {
            source_name: file.source_name.clone(),
            source_content: file.content.clone(),
            contract: Some(contract.name.clone()),
            function: Some(CONSTRUCTOR_FUNCTION_NAME.to_string()),
            line,
            range: [
                contract_location.offset,
                contract_location.offset + contract_location.length,
            ]
            .to_vec(),
        })
    }

    fn is_constructor_invalid_arguments_error(trace: &CreateMessageTrace) -> napi::Result<bool> {
        if trace.return_data.len() > 0 {
            return Ok(false);
        }

        let bytecode = trace.bytecode.as_ref().expect("JS code asserts");
        let contract = bytecode.contract.borrow();

        // This function is only matters with contracts that have constructors defined.
        // The ones that don't are abstract contracts, or their constructor
        // doesn't take any argument.
        let Some(constructor) = &contract.constructor else {
            return Ok(false);
        };

        let Ok(version) = Version::parse(&bytecode.compiler_version) else {
            return Ok(false);
        };
        if version < FIRST_SOLC_VERSION_CREATE_PARAMS_VALIDATION {
            return Ok(false);
        }

        let last_step = trace.steps.last();
        let Some(Either4::A(last_step)) = last_step else {
            return Ok(false);
        };

        let last_inst = bytecode.get_instruction(last_step.pc)?;

        if last_inst.opcode != OpCode::REVERT || last_inst.location.is_some() {
            return Ok(false);
        }

        let mut has_read_deployment_code_size = false;
        for step in &trace.steps {
            let step = match step {
                Either4::A(step) => step,
                _ => return Ok(false),
            };

            let inst = bytecode.get_instruction(step.pc)?;

            if let Some(inst_location) = &inst.location {
                if contract.location != *inst_location && constructor.location != *inst_location {
                    return Ok(false);
                }
            }

            if inst.opcode == OpCode::CODESIZE {
                has_read_deployment_code_size = true;
            }
        }

        Ok(has_read_deployment_code_size)
    }

    fn get_contract_start_without_function_source_reference(
        trace: Either<&CallMessageTrace, &CreateMessageTrace>,
    ) -> napi::Result<SourceReference> {
        let bytecode = match &trace {
            Either::A(create) => &create.bytecode,
            Either::B(call) => &call.bytecode,
        };

        let contract = &bytecode.as_ref().expect("JS code asserts").contract;

        let contract = contract.borrow();

        let location = &contract.location;
        let file = location.file();
        let file = file.borrow();

        Ok(SourceReference {
            source_name: file.source_name.clone(),
            source_content: file.content.clone(),
            contract: Some(contract.name.clone()),

            function: None,
            line: location.get_starting_line_number(),
            range: [location.offset, location.offset + location.length].to_vec(),
        })
    }

    fn is_function_not_payable_error(
        trace: &CallMessageTrace,
        called_function: &ContractFunction,
    ) -> napi::Result<bool> {
        // This error doesn't return data
        if !trace.return_data.is_empty() {
            return Ok(false);
        }

        let (neg, value, _) = trace.value.get_u64();
        if neg || value == 0 {
            return Ok(false);
        }

        let bytecode = trace.bytecode.as_ref().expect("JS code asserts");
        let contract = bytecode.contract.borrow();

        // Libraries don't have a nonpayable check
        if contract.r#type == ContractKind::Library {
            return Ok(false);
        }

        Ok(called_function.is_payable != Some(true))
    }

    fn get_last_source_reference(
        trace: Either<&CallMessageTrace, &CreateMessageTrace>,
    ) -> napi::Result<Option<SourceReference>> {
        let (bytecode, steps) = match trace {
            Either::A(create) => (&create.bytecode, &create.steps),
            Either::B(call) => (&call.bytecode, &call.steps),
        };

        let bytecode = bytecode
            .as_ref()
            .expect("JS code only accepted variants that had bytecode defined");

        for step in steps.iter().rev() {
            let step = match step {
                Either4::A(step) => step,
                _ => continue,
            };

            let inst = bytecode.get_instruction(step.pc)?;

            let Some(location) = &inst.location else {
                continue;
            };

            let source_reference = source_location_to_source_reference(bytecode, Some(location))?;

            if let Some(source_reference) = source_reference {
                return Ok(Some(source_reference));
            }
        }

        Ok(None)
    }

    fn has_failed_inside_the_fallback_function(trace: &CallMessageTrace) -> napi::Result<bool> {
        let contract = &trace.bytecode.as_ref().expect("JS code asserts").contract;
        let contract = contract.borrow();

        match &contract.fallback {
            Some(fallback) => Self::has_failed_inside_function(trace, fallback),
            None => Ok(false),
        }
    }

    fn has_failed_inside_the_receive_function(trace: &CallMessageTrace) -> napi::Result<bool> {
        let contract = &trace.bytecode.as_ref().expect("JS code asserts").contract;
        let contract = contract.borrow();

        match &contract.receive {
            Some(receive) => Self::has_failed_inside_function(trace, receive),
            None => Ok(false),
        }
    }

    fn has_failed_inside_function(
        trace: &CallMessageTrace,
        func: &ContractFunction,
    ) -> napi::Result<bool> {
        let last_step = trace
            .steps
            .last()
            .expect("There should at least be one step");

        let last_step = match last_step {
            Either4::A(step) => step,
            _ => panic!("JS code asserted this is always an EvmStep"),
        };

        let last_instruction = trace
            .bytecode
            .as_ref()
            .expect("The TS code type-checks this to always have bytecode")
            .get_instruction(last_step.pc)?;

        Ok(match &last_instruction.location {
            Some(last_instruction_location) => {
                last_instruction.opcode == OpCode::REVERT
                    && func.location.contains(last_instruction_location)
            }
            _ => false,
        })
    }

    fn instruction_within_function_to_revert_stack_trace_entry(
        trace: Either<&CallMessageTrace, &CreateMessageTrace>,
        inst: &Instruction,
    ) -> napi::Result<RevertErrorStackTraceEntry> {
        let bytecode = match &trace {
            Either::A(create) => &create.bytecode,
            Either::B(call) => &call.bytecode,
        }
        .as_ref()
        .expect("JS code asserts");

        let source_reference =
            source_location_to_source_reference(bytecode, inst.location.as_deref())?
                .expect("Expected source reference to be defined");

        let return_data = match &trace {
            Either::A(create) => &create.return_data,
            Either::B(call) => &call.return_data,
        };

        Ok(RevertErrorStackTraceEntry {
            type_: StackTraceEntryTypeConst,
            source_reference,
            is_invalid_opcode_error: inst.opcode == OpCode::INVALID,
            return_data: return_data.clone(),
        })
    }

    fn instruction_within_function_to_unmapped_solc_0_6_3_revert_error_stack_trace_entry(
        trace: Either<&CallMessageTrace, &CreateMessageTrace>,
        inst: &Instruction,
    ) -> napi::Result<UnmappedSolc063RevertErrorStackTraceEntry> {
        let bytecode = match &trace {
            Either::A(create) => &create.bytecode,
            Either::B(call) => &call.bytecode,
        }
        .as_ref()
        .expect("JS code asserts");

        let source_reference =
            source_location_to_source_reference(bytecode, inst.location.as_deref())?;

        Ok(UnmappedSolc063RevertErrorStackTraceEntry {
            type_: StackTraceEntryTypeConst,
            source_reference,
        })
    }

    fn instruction_within_function_to_panic_stack_trace_entry(
        trace: Either<&CallMessageTrace, &CreateMessageTrace>,
        inst: &Instruction,
        error_code: BigInt,
    ) -> napi::Result<PanicErrorStackTraceEntry> {
        let last_source_reference = Self::get_last_source_reference(trace)?;

        let bytecode = match &trace {
            Either::A(create) => &create.bytecode,
            Either::B(call) => &call.bytecode,
        }
        .as_ref()
        .expect("JS code asserts");

        let source_reference =
            source_location_to_source_reference(bytecode, inst.location.as_deref())?;

        let source_reference = source_reference.or(last_source_reference);

        Ok(PanicErrorStackTraceEntry {
            type_: StackTraceEntryTypeConst,
            source_reference,
            error_code,
        })
    }

    fn instruction_within_function_to_custom_error_stack_trace_entry(
        trace: Either<&CallMessageTrace, &CreateMessageTrace>,
        inst: &Instruction,
        message: String,
    ) -> napi::Result<CustomErrorStackTraceEntry> {
        let last_source_reference = Self::get_last_source_reference(trace)?;
        let last_source_reference =
            last_source_reference.expect("Expected source reference to be defined");

        let bytecode = match &trace {
            Either::A(create) => &create.bytecode,
            Either::B(call) => &call.bytecode,
        }
        .as_ref()
        .expect("JS code asserts");

        let source_reference =
            source_location_to_source_reference(bytecode, inst.location.as_deref())?;

        let source_reference = source_reference.unwrap_or(last_source_reference);

        Ok(CustomErrorStackTraceEntry {
            type_: StackTraceEntryTypeConst,
            source_reference,
            message,
        })
    }

    fn solidity_0_6_3_maybe_unmapped_revert(
        trace: Either<&CallMessageTrace, &CreateMessageTrace>,
    ) -> napi::Result<bool> {
        let (bytecode, steps) = match &trace {
            Either::A(create) => (&create.bytecode, &create.steps),
            Either::B(call) => (&call.bytecode, &call.steps),
        };

        let bytecode = bytecode
            .as_ref()
            .expect("JS code only accepted variants that had bytecode defined");

        if steps.is_empty() {
            return Ok(false);
        }

        let last_step = steps.last();
        let last_step = match last_step {
            Some(Either4::A(step)) => step,
            _ => return Ok(false),
        };

        let last_instruction = bytecode.get_instruction(last_step.pc)?;

        let Ok(version) = Version::parse(&bytecode.compiler_version) else {
            return Ok(false);
        };
        let req = VersionReq::parse(&format!("^{FIRST_SOLC_VERSION_WITH_UNMAPPED_REVERTS}"))
            .expect("valid semver");

        Ok(req.matches(&version) && last_instruction.opcode == OpCode::REVERT)
    }

    // Solidity 0.6.3 unmapped reverts special handling
    // For more info: https://github.com/ethereum/solidity/issues/9006
    fn solidity_0_6_3_get_frame_for_unmapped_revert_before_function(
        trace: &CallMessageTrace,
    ) -> napi::Result<Option<UnmappedSolc063RevertErrorStackTraceEntry>> {
        let bytecode = trace.bytecode.as_ref().expect("JS code asserts");
        let contract = bytecode.contract.borrow();

        let revert_frame =
            Self::solidity_0_6_3_get_frame_for_unmapped_revert_within_function(Either::A(trace))?;

        let revert_frame = match revert_frame {
            None
            | Some(UnmappedSolc063RevertErrorStackTraceEntry {
                source_reference: None,
                ..
            }) => {
                if contract.receive.is_none() || trace.calldata.len() > 0 {
                    // Failed within the fallback
                    if let Some(fallback) = &contract.fallback {
                        let location = &fallback.location;
                        let file = location.file();
                        let file = file.borrow();

                        let revert_frame = UnmappedSolc063RevertErrorStackTraceEntry {
                            type_: StackTraceEntryTypeConst,
                            source_reference: Some(SourceReference {
                                contract: Some(contract.name.clone()),
                                function: Some(FALLBACK_FUNCTION_NAME.to_string()),
                                source_name: file.source_name.clone(),
                                source_content: file.content.clone(),
                                line: location.get_starting_line_number(),
                                range: [location.offset, location.offset + location.length]
                                    .to_vec(),
                            }),
                        };

                        Some(Self::solidity_0_6_3_correct_line_number(revert_frame))
                    } else {
                        None
                    }
                } else {
                    let receive = contract
                        .receive
                        .as_ref()
                        .expect("None always hits branch above");

                    let location = &receive.location;
                    let file = location.file();
                    let file = file.borrow();

                    let revert_frame = UnmappedSolc063RevertErrorStackTraceEntry {
                        type_: StackTraceEntryTypeConst,
                        source_reference: Some(SourceReference {
                            contract: Some(contract.name.clone()),
                            function: Some(RECEIVE_FUNCTION_NAME.to_string()),
                            source_name: file.source_name.clone(),
                            source_content: file.content.clone(),
                            line: location.get_starting_line_number(),
                            range: [location.offset, location.offset + location.length].to_vec(),
                        }),
                    };

                    Some(Self::solidity_0_6_3_correct_line_number(revert_frame))
                }
            }
            Some(revert_frame) => Some(revert_frame),
        };

        Ok(revert_frame)
    }

    fn solidity_0_6_3_get_frame_for_unmapped_revert_within_function(
        trace: Either<&CallMessageTrace, &CreateMessageTrace>,
    ) -> napi::Result<Option<UnmappedSolc063RevertErrorStackTraceEntry>> {
        let (bytecode, steps) = match &trace {
            Either::A(create) => (&create.bytecode, &create.steps),
            Either::B(call) => (&call.bytecode, &call.steps),
        };

        let bytecode = bytecode
            .as_ref()
            .expect("JS code only accepted variants that had bytecode defined");

        let contract = bytecode.contract.borrow();

        // If we are within a function there's a last valid location. It may
        // be the entire contract.
        let prev_inst = Self::get_last_instruction_with_valid_location(trace)?;
        let last_step = match steps.last() {
            Some(Either4::A(step)) => step,
            _ => panic!("JS code asserts this is always an EvmStep"),
        };
        let next_inst_pc = last_step.pc + 1;
        let has_next_inst = bytecode.has_instruction(next_inst_pc);

        if has_next_inst {
            let next_inst = bytecode.get_instruction(next_inst_pc)?;

            let prev_loc = prev_inst.and_then(|i| i.location.as_deref());
            let next_loc = next_inst.location.as_deref();

            let prev_func = prev_loc.and_then(SourceLocation::get_containing_function);
            let next_func = next_loc.and_then(SourceLocation::get_containing_function);

            // This is probably a require. This means that we have the exact
            // line, but the stack trace may be degraded (e.g. missing our
            // synthetic call frames when failing in a modifier) so we still
            // add this frame as UNMAPPED_SOLC_0_6_3_REVERT_ERROR
            match (&prev_func, &next_loc, &prev_loc) {
                (Some(_), Some(next_loc), Some(prev_loc)) if prev_loc == next_loc => {
                    return Ok(Some(Self::instruction_within_function_to_unmapped_solc_0_6_3_revert_error_stack_trace_entry(
                trace,
                next_inst,

              )?));
                }
                _ => {}
            }

            let revert_frame = if prev_func.is_some() && prev_inst.is_some() {
                Some(Self::instruction_within_function_to_unmapped_solc_0_6_3_revert_error_stack_trace_entry(
                trace,
                prev_inst.as_ref().unwrap(),

              )?)
            } else if next_func.is_some() {
                Some(Self::instruction_within_function_to_unmapped_solc_0_6_3_revert_error_stack_trace_entry(
                trace,
                next_inst,

              )?)
            } else {
                None
            };

            return Ok(revert_frame.map(Self::solidity_0_6_3_correct_line_number));
        }

        if matches!(trace, Either::B(CreateMessageTrace { .. })) && prev_inst.is_some() {
            // Solidity is smart enough to stop emitting extra instructions after
            // an unconditional revert happens in a constructor. If this is the case
            // we just return a special error.

            let mut constructor_revert_frame = Self::instruction_within_function_to_unmapped_solc_0_6_3_revert_error_stack_trace_entry(
                trace,
                prev_inst.as_ref().unwrap(),

            )?;

            // When the latest instruction is not within a function we need
            // some default sourceReference to show to the user
            if constructor_revert_frame.source_reference.is_none() {
                let location = &contract.location;
                let file = location.file();
                let file = file.borrow();

                let mut default_source_reference = SourceReference {
                    function: Some(CONSTRUCTOR_FUNCTION_NAME.to_string()),
                    contract: Some(contract.name.clone()),
                    source_name: file.source_name.clone(),
                    source_content: file.content.clone(),
                    line: location.get_starting_line_number(),
                    range: [location.offset, location.offset + location.length].to_vec(),
                };

                if let Some(constructor) = &contract.constructor {
                    default_source_reference.line = constructor.location.get_starting_line_number();
                }

                constructor_revert_frame.source_reference = Some(default_source_reference);
            } else {
                constructor_revert_frame =
                    Self::solidity_0_6_3_correct_line_number(constructor_revert_frame);
            }

            return Ok(Some(constructor_revert_frame));
        }

        if let Some(prev_inst) = prev_inst {
            // We may as well just be in a function or modifier and just happen
            // to be at the last instruction of the runtime bytecode.
            // In this case we just return whatever the last mapped intruction
            // points to.
            let mut latest_instruction_revert_frame = Self::instruction_within_function_to_unmapped_solc_0_6_3_revert_error_stack_trace_entry(
                trace,
                prev_inst,

            )?;

            if latest_instruction_revert_frame.source_reference.is_some() {
                latest_instruction_revert_frame =
                    Self::solidity_0_6_3_correct_line_number(latest_instruction_revert_frame);
            }
            return Ok(Some(latest_instruction_revert_frame));
        }

        Ok(None)
    }

    fn solidity_0_6_3_correct_line_number(
        mut revert_frame: UnmappedSolc063RevertErrorStackTraceEntry,
    ) -> UnmappedSolc063RevertErrorStackTraceEntry {
        let Some(source_reference) = &mut revert_frame.source_reference else {
            return revert_frame;
        };

        let lines: Vec<_> = source_reference.source_content.split('\n').collect();

        let current_line = lines[source_reference.line as usize - 1];
        if current_line.contains("require") || current_line.contains("revert") {
            return revert_frame;
        }

        let next_lines = &lines
            .get(source_reference.line as usize..)
            .unwrap_or_default();
        let first_non_empty_line = next_lines.iter().position(|l| !l.trim().is_empty());

        let Some(first_non_empty_line) = first_non_empty_line else {
            return revert_frame;
        };

        let next_line = next_lines[first_non_empty_line];
        if next_line.contains("require") || next_line.contains("revert") {
            source_reference.line += 1 + first_non_empty_line as u32;
        }

        revert_frame
    }

    fn get_other_error_before_called_function_stack_trace_entry(
        trace: &CallMessageTrace,
    ) -> napi::Result<OtherExecutionErrorStackTraceEntry> {
        let source_reference =
            Self::get_contract_start_without_function_source_reference(Either::A(trace))?;

        Ok(OtherExecutionErrorStackTraceEntry {
            type_: StackTraceEntryTypeConst,
            source_reference: Some(source_reference),
        })
    }

    fn is_called_non_contract_account_error(
        trace: Either<&CallMessageTrace, &CreateMessageTrace>,
    ) -> napi::Result<bool> {
        // We could change this to checking that the last valid location maps to a call,
        // but it's way more complex as we need to get the ast node from that
        // location.

        let (bytecode, steps) = match &trace {
            Either::A(create) => (&create.bytecode, &create.steps),
            Either::B(call) => (&call.bytecode, &call.steps),
        };

        let bytecode = bytecode
            .as_ref()
            .expect("JS code only accepted variants that had bytecode defined");

        let last_index = Self::get_last_instruction_with_valid_location_step_index(trace)?;

        let last_index = match last_index {
            None | Some(0) => return Ok(false),
            Some(last_index) => last_index as usize,
        };

        let last_step = match &steps[last_index] {
            Either4::A(step) => step,
            _ => panic!("We know this is an EVM step"),
        };

        let last_inst = bytecode.get_instruction(last_step.pc)?;

        if last_inst.opcode != OpCode::ISZERO {
            return Ok(false);
        }

        let prev_step = match &steps[last_index - 1] {
            Either4::A(step) => step,
            _ => panic!("We know this is an EVM step"),
        };

        let prev_inst = bytecode.get_instruction(prev_step.pc)?;

        Ok(prev_inst.opcode == OpCode::EXTCODESIZE)
    }

    fn get_last_instruction_with_valid_location_step_index(
        trace: Either<&CallMessageTrace, &CreateMessageTrace>,
    ) -> napi::Result<Option<u32>> {
        let (bytecode, steps) = match &trace {
            Either::A(create) => (&create.bytecode, &create.steps),
            Either::B(call) => (&call.bytecode, &call.steps),
        };

        let bytecode = bytecode
            .as_ref()
            .expect("JS code only accepted variants that had bytecode defined");

        for (i, step) in steps.iter().enumerate().rev() {
            let step = match step {
                Either4::A(step) => step,
                _ => return Ok(None),
            };

            let inst = bytecode.get_instruction(step.pc)?;

            if inst.location.is_some() {
                return Ok(Some(i as u32));
            }
        }

        Ok(None)
    }

    fn get_last_instruction_with_valid_location<'a>(
        trace: Either<&'a CallMessageTrace, &'a CreateMessageTrace>,
    ) -> napi::Result<Option<&'a Instruction>> {
        let last_location_index = Self::get_last_instruction_with_valid_location_step_index(trace)?;

        let Some(last_location_index) = last_location_index else {
            return Ok(None);
        };

        let (bytecode, steps) = match &trace {
            Either::A(create) => (&create.bytecode, &create.steps),
            Either::B(call) => (&call.bytecode, &call.steps),
        };

        let bytecode = bytecode
            .as_ref()
            .expect("JS code only accepted variants that had bytecode defined");

        match &steps.get(last_location_index as usize) {
            Some(Either4::A(step)) => {
                let inst = bytecode.get_instruction(step.pc)?;

                Ok(Some(inst))
            }
            _ => Ok(None),
        }
    }
}

fn source_location_to_source_reference(
    bytecode: &Bytecode,
    location: Option<&SourceLocation>,
) -> napi::Result<Option<SourceReference>> {
    let Some(location) = location else {
        return Ok(None);
    };
    let Some(func) = location.get_containing_function() else {
        return Ok(None);
    };

    let func_name = match func.r#type {
        ContractFunctionType::Constructor => CONSTRUCTOR_FUNCTION_NAME.to_string(),
        ContractFunctionType::Fallback => FALLBACK_FUNCTION_NAME.to_string(),
        ContractFunctionType::Receive => RECEIVE_FUNCTION_NAME.to_string(),
        _ => func.name.clone(),
    };

    let func_location_file = func.location.file();
    let func_location_file = func_location_file.borrow();

    Ok(Some(SourceReference {
        function: Some(func_name.clone()),
        contract: if func.r#type == ContractFunctionType::FreeFunction {
            None
        } else {
            Some(bytecode.contract.borrow().name.clone())
        },
        source_name: func_location_file.source_name.clone(),
        source_content: func_location_file.content.clone(),
        line: location.get_starting_line_number(),
        range: [location.offset, location.offset + location.length].to_vec(),
    }))
}

pub fn instruction_to_callstack_stack_trace_entry(
    bytecode: &Bytecode,
    inst: &Instruction,
) -> napi::Result<Either<CallstackEntryStackTraceEntry, InternalFunctionCallStackEntry>> {
    let contract = bytecode.contract.borrow();

    // This means that a jump is made from within an internal solc function.
    // These are normally made from yul code, so they don't map to any Solidity
    // function
    let inst_location = match &inst.location {
        None => {
            let location = &contract.location;
            let file = location.file();
            let file = file.borrow();

            return Ok(Either::B(InternalFunctionCallStackEntry {
                type_: StackTraceEntryTypeConst,
                pc: inst.pc,
                source_reference: SourceReference {
                    source_name: file.source_name.clone(),
                    source_content: file.content.clone(),
                    contract: Some(contract.name.clone()),
                    function: None,
                    line: location.get_starting_line_number(),
                    range: [location.offset, location.offset + location.length].to_vec(),
                },
            }));
        }
        Some(inst_location) => inst_location,
    };

    if let Some(func) = inst_location.get_containing_function() {
        let source_reference = source_location_to_source_reference(bytecode, Some(inst_location))?
            .expect("Expected source reference to be defined");

        return Ok(Either::A(CallstackEntryStackTraceEntry {
            type_: StackTraceEntryTypeConst,
            source_reference,
            function_type: func.r#type.into(),
        }));
    };

    let file = inst_location.file();
    let file = file.borrow();

    Ok(Either::A(CallstackEntryStackTraceEntry {
        type_: StackTraceEntryTypeConst,
        source_reference: SourceReference {
            function: None,
            contract: Some(contract.name.clone()),
            source_name: file.source_name.clone(),
            source_content: file.content.clone(),
            line: inst_location.get_starting_line_number(),
            range: [
                inst_location.offset,
                inst_location.offset + inst_location.length,
            ]
            .to_vec(),
        },
        function_type: ContractFunctionType::Function.into(),
    }))
}

// Rewrite of `AbiHelpers.formatValues` from Hardhat
fn format_dyn_sol_value(val: &DynSolValue) -> String {
    match val {
        // print nested values as [value1, value2, ...]
        DynSolValue::Array(items)
        | DynSolValue::Tuple(items)
        | DynSolValue::FixedArray(items)
        | DynSolValue::CustomStruct { tuple: items, .. } => {
            let mut result = String::from("[");
            for (i, val) in items.iter().enumerate() {
                if i > 0 {
                    result.push_str(", ");
                }
                result.push_str(&format_dyn_sol_value(val));
            }

            result.push(']');
            result
        }
        // surround string values with quotes
        DynSolValue::String(s) => format!("\"{s}\""),

        DynSolValue::Address(address) => format!("\"{address}\""),
        DynSolValue::Bytes(bytes) => format!("\"{}\"", hex::encode_prefixed(bytes)),
        DynSolValue::FixedBytes(word, size) => {
            format!("\"{}\"", hex::encode_prefixed(&word.0.as_slice()[..*size]))
        }
        DynSolValue::Bool(b) => b.to_string(),
        DynSolValue::Function(_) => "<function>".to_string(),
        DynSolValue::Int(int, _bits) => int.to_string(),
        DynSolValue::Uint(uint, _bits) => uint.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sol_value_to_string() {
        assert_eq!(
            format_dyn_sol_value(&DynSolValue::String("hello".to_string())),
            "\"hello\""
        );
        // Uniform, 0-prefixed hex strings
        assert_eq!(
            format_dyn_sol_value(&DynSolValue::Address([0u8; 20].into())),
            format!(r#""0x{}""#, "0".repeat(2 * 20))
        );
        assert_eq!(
            format_dyn_sol_value(&DynSolValue::Bytes(vec![0u8; 32])),
            format!(r#""0x{}""#, "0".repeat(2 * 32))
        );
        assert_eq!(
            format_dyn_sol_value(&DynSolValue::FixedBytes([0u8; 32].into(), 10)),
            format!(r#""0x{}""#, "0".repeat(2 * 10))
        );
        assert_eq!(
            format_dyn_sol_value(&DynSolValue::FixedBytes([0u8; 32].into(), 32)),
            format!(r#""0x{}""#, "0".repeat(2 * 32))
        );
    }
}
