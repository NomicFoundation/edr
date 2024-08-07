use std::collections::HashSet;

use alloy_dyn_abi::{DynSolValue, JsonAbiExt};
use edr_evm::hex;
use napi::{
    bindgen_prelude::{BigInt, Either24, Either3, Either4, Uint8Array, Undefined},
    Either, Env,
};
use napi_derive::napi;
use semver::{Version, VersionReq};

use crate::{
    trace::{
        model::{ContractFunctionType, Instruction, JumpType},
        solidity_stack_trace::{
            ContractCallRunOutOfGasError, ContractTooLargeErrorStackTraceEntry,
            DirectLibraryCallErrorStackTraceEntry,
            FallbackNotPayableAndNoReceiveErrorStackTraceEntry,
            FunctionNotPayableErrorStackTraceEntry, MissingFallbackOrReceiveErrorStackTraceEntry,
            OtherExecutionErrorStackTraceEntry, ReturndataSizeErrorStackTraceEntry,
            RevertErrorStackTraceEntry, StackTraceEntryTypeConst,
            UnrecognizedFunctionWithoutFallbackErrorStackTraceEntry, CONSTRUCTOR_FUNCTION_NAME,
            FALLBACK_FUNCTION_NAME, RECEIVE_FUNCTION_NAME,
        },
    },
    utils::ClassInstanceRef,
};

use super::{
    exit::ExitCode,
    message_trace::{CallMessageTrace, CreateMessageTrace, EvmStep, PrecompileMessageTrace},
    model::{Bytecode, ContractFunction, ContractType, SourceLocation},
    opcodes::Opcode,
    return_data::ReturnData,
    solidity_stack_trace::{
        CallFailedErrorStackTraceEntry, CallstackEntryStackTraceEntry, CustomErrorStackTraceEntry,
        FallbackNotPayableErrorStackTraceEntry, InternalFunctionCallStackEntry,
        InvalidParamsErrorStackTraceEntry, NonContractAccountCalledErrorStackTraceEntry,
        PanicErrorStackTraceEntry, SolidityStackTrace, SolidityStackTraceEntry,
        SolidityStackTraceEntryExt, SourceReference, UnmappedSolc063RevertErrorStackTraceEntry,
    },
};

pub(crate) trait AsEitherRef {
    type Left;
    type Right;

    fn as_ref(&self) -> Either<&Self::Left, &Self::Right>;
}

impl<A, B> AsEitherRef for Either<A, B> {
    type Left = A;
    type Right = B;

    fn as_ref(&self) -> Either<&A, &B> {
        match self {
            Either::A(ref a) => Either::A(a),
            Either::B(ref b) => Either::B(b),
        }
    }
}

pub enum ModifiedStackTrace {
    Yes(SolidityStackTrace),
    No(SolidityStackTrace),
}

impl ModifiedStackTrace {
    pub fn into_inner(self) -> SolidityStackTrace {
        match self {
            ModifiedStackTrace::No(stack) | ModifiedStackTrace::Yes(stack) => stack,
        }
    }
}

trait IntoEither<T> {
    fn into_either(self) -> Either<T, Undefined>;
}

impl<T> IntoEither<T> for Option<T> {
    fn into_either(self) -> Either<T, Undefined> {
        match self {
            Some(a) => Either::A(a),
            None => Either::B(()),
        }
    }
}

trait IntoOption<T> {
    fn into_option(self) -> Option<T>;
}

impl<T> IntoOption<T> for Either<T, Undefined> {
    fn into_option(self) -> Option<T> {
        match self {
            Either::A(a) => Some(a),
            Either::B(_) => None,
        }
    }
}

const FIRST_SOLC_VERSION_CREATE_PARAMS_VALIDATION: Version = Version::new(0, 5, 9);
const FIRST_SOLC_VERSION_RECEIVE_FUNCTION: Version = Version::new(0, 6, 0);
const FIRST_SOLC_VERSION_WITH_UNMAPPED_REVERTS: &str = "0.6.3";

#[napi(object)]
pub struct SubmessageData {
    pub message_trace: Either3<PrecompileMessageTrace, CallMessageTrace, CreateMessageTrace>,
    pub stacktrace: SolidityStackTrace,
    pub step_index: u32,
}

#[napi]
pub struct ErrorInferrer;

#[napi]
impl ErrorInferrer {
    #[napi]
    pub fn infer_before_tracing_call_message(
        trace: CallMessageTrace,
        env: Env,
    ) -> napi::Result<Either<SolidityStackTrace, Undefined>> {
        if Self::is_direct_library_call(&trace, env)? {
            return Ok(Either::A(Self::get_direct_library_call_error_stack_trace(
                &trace, env,
            )?));
        }

        let bytecode = trace
            .bytecode
            .as_ref()
            .expect("The TS code type-checks this to always have bytecode");
        let contract = bytecode.contract.borrow(env)?;

        let called_function = contract.get_function_from_selector_inner(
            trace.calldata.get(..4).unwrap_or(&trace.calldata[..]),
        );

        if let Some(called_function) = called_function {
            let called_function = called_function.borrow(env)?;

            if Self::is_function_not_payable_error(&trace, &called_function, env)? {
                return Ok(Either::A(vec![FunctionNotPayableErrorStackTraceEntry {
                    type_: StackTraceEntryTypeConst,
                    source_reference: Self::get_function_start_source_reference_inner(
                        Either::A(&trace),
                        &called_function,
                        env,
                    )?,
                    value: trace.value,
                }
                .into()]));
            }
        }

        let called_function = called_function.map(|x| x.borrow(env)).transpose()?;
        let called_function = called_function.as_deref();

        if Self::is_missing_function_and_fallback_error(&trace, called_function, env)? {
            let source_reference =
                Self::get_contract_start_without_function_source_reference_inner(
                    Either::A(&trace),
                    env,
                )?;

            if Self::empty_calldata_and_no_receive(&trace, env)? {
                return Ok(Either::A(vec![
                    MissingFallbackOrReceiveErrorStackTraceEntry {
                        type_: StackTraceEntryTypeConst,
                        source_reference,
                    }
                    .into(),
                ]));
            }

            return Ok(Either::A(vec![
                UnrecognizedFunctionWithoutFallbackErrorStackTraceEntry {
                    type_: StackTraceEntryTypeConst,
                    source_reference,
                }
                .into(),
            ]));
        }

        if Self::is_fallback_not_payable_error(&trace, called_function, env)? {
            let source_reference = Self::get_fallback_start_source_reference_inner(&trace, env)?;

            if Self::empty_calldata_and_no_receive(&trace, env)? {
                return Ok(Either::A(vec![
                    FallbackNotPayableAndNoReceiveErrorStackTraceEntry {
                        type_: StackTraceEntryTypeConst,
                        source_reference,
                        value: trace.value,
                    }
                    .into(),
                ]));
            }

            return Ok(Either::A(vec![FallbackNotPayableErrorStackTraceEntry {
                type_: StackTraceEntryTypeConst,
                source_reference,
                value: trace.value,
            }
            .into()]));
        }

        Ok(Either::B(()))
    }

    #[napi]
    pub fn infer_before_tracing_create_message(
        trace: CreateMessageTrace,
        env: Env,
    ) -> napi::Result<Either<SolidityStackTrace, Undefined>> {
        if Self::is_constructor_not_payable_error(&trace, env)? {
            return Ok(Either::A(vec![FunctionNotPayableErrorStackTraceEntry {
                type_: StackTraceEntryTypeConst,
                source_reference: Self::get_constructor_start_source_reference_inner(&trace, env)?,
                value: trace.value,
            }
            .into()]));
        }

        if Self::is_constructor_invalid_arguments_error(&trace, env)? {
            return Ok(Either::A(vec![InvalidParamsErrorStackTraceEntry {
                type_: StackTraceEntryTypeConst,
                source_reference: Self::get_constructor_start_source_reference_inner(&trace, env)?,
            }
            .into()]));
        }

        Ok(Either::B(()))
    }

    #[napi]
    pub fn infer_after_tracing(
        trace: Either<CallMessageTrace, CreateMessageTrace>,
        stacktrace: SolidityStackTrace,
        function_jumpdests: Vec<&Instruction>,
        jumped_into_function: bool,
        last_submessage_data: Either<SubmessageData, Undefined>,
        env: Env,
    ) -> napi::Result<Either<SolidityStackTrace, Undefined>> {
        macro_rules! propagate_stacktrace {
            ($stacktrace:ident, $res: ident) => {
                let $stacktrace = match $res {
                    ModifiedStackTrace::Yes(stacktrace) => return Ok(Either::A(stacktrace)),
                    ModifiedStackTrace::No(stacktrace) => stacktrace,
                };
            };
        }
        // FIXME: Calling these in JS somehow works, because it seems that it's
        // copying the values around between passes?
        // We need to make this work in Rust as well; if we stop needing
        // to export return data from the trace objects, then we can make this
        // clonable and just clone it here
        // OR: we probably need to wrap all of these frames in Rc and clone it
        // this way
        let trace = trace.as_ref();

        let res = Self::check_last_submessage_inner(trace, stacktrace, last_submessage_data, env)?;
        propagate_stacktrace!(stacktrace, res);

        let res = Self::check_failed_last_call_inner(trace, stacktrace, env)?;
        propagate_stacktrace!(stacktrace, res);

        let res = Self::check_last_instruction_inner(
            trace,
            stacktrace,
            function_jumpdests.clone(),
            jumped_into_function,
            env,
        )?;
        propagate_stacktrace!(stacktrace, res);

        let res = Self::check_non_contract_called_inner(trace, stacktrace, env)?;
        propagate_stacktrace!(stacktrace, res);

        let res = Self::check_solidity_0_6_3_unmapped_revert_inner(trace, stacktrace, env)?;
        propagate_stacktrace!(stacktrace, res);

        // NOTE: We added the stacktrace for context threading here
        let res = Self::check_contract_too_large_inner(trace, stacktrace, env)?;
        propagate_stacktrace!(stacktrace, res);

        let res = Self::other_execution_error_stacktrace_inner(trace, stacktrace, env)?;
        let res = Either::A(res);

        Ok(res)
    }

    #[napi]
    pub fn filter_redundant_frames(
        stacktrace: SolidityStackTrace,
    ) -> napi::Result<SolidityStackTrace> {
        // To work around the borrow checker, we'll collect the indices of the frames we want to keep
        // We can't clone the frames, because some of them contain non-Clone `ClassInstance`s`
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
                            // TODO: JS also checked that it's not undefined but it seems it never is?
                            // looking at the types
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

    #[napi]
    pub fn check_contract_too_large(
        trace: Either<CallMessageTrace, CreateMessageTrace>,
        env: Env,
    ) -> napi::Result<Either<SolidityStackTrace, Undefined>> {
        Ok(match trace {
            Either::B(create @ CreateMessageTrace { .. })
                if create.exit.kind() == ExitCode::CODESIZE_EXCEEDS_MAXIMUM =>
            {
                Either::A(vec![ContractTooLargeErrorStackTraceEntry {
                    type_: StackTraceEntryTypeConst,
                    source_reference: Some(Self::get_constructor_start_source_reference_inner(
                        &create, env,
                    )?),
                }
                .into()])
            }

            _ => Either::B(()),
        })
    }

    pub fn check_contract_too_large_inner(
        trace: Either<&CallMessageTrace, &CreateMessageTrace>,
        stacktrace: SolidityStackTrace,
        env: Env,
    ) -> napi::Result<ModifiedStackTrace> {
        Ok(match trace {
            Either::B(create @ CreateMessageTrace { .. })
                if create.exit.kind() == ExitCode::CODESIZE_EXCEEDS_MAXIMUM =>
            {
                ModifiedStackTrace::Yes(vec![ContractTooLargeErrorStackTraceEntry {
                    type_: StackTraceEntryTypeConst,
                    source_reference: Some(Self::get_constructor_start_source_reference_inner(
                        create, env,
                    )?),
                }
                .into()])
            }

            _ => ModifiedStackTrace::No(stacktrace),
        })
    }

    /// Check if the last call/create that was done failed.
    #[napi]
    pub fn check_failed_last_call(
        trace: Either<CallMessageTrace, CreateMessageTrace>,
        stacktrace: SolidityStackTrace,
        env: Env,
    ) -> napi::Result<Either<SolidityStackTrace, Undefined>> {
        let (bytecode, steps) = match &trace {
            Either::A(call) => (&call.bytecode, &call.steps),
            Either::B(create) => (&create.bytecode, &create.steps),
        };

        let bytecode = bytecode.as_ref().expect("JS code asserts");

        for step_index in (0..steps.len() - 1).rev() {
            let (step, next_step) = match &steps[step_index..][..2] {
                &[Either4::A(ref step), ref next_step] => (step, next_step),
                _ => return Ok(Either::B(())),
            };

            let inst = bytecode.get_instruction_inner(step.pc)?;
            let inst = inst.borrow(env)?;

            if let (Opcode::CALL | Opcode::CREATE, Either4::A(EvmStep { .. })) =
                (inst.opcode, next_step)
            {
                if Self::is_call_failed_error(trace.as_ref(), step_index as u32, &inst, env)? {
                    let mut stacktrace = stacktrace;
                    stacktrace.push(
                        Self::call_instruction_to_call_failed_to_execute_stack_trace_entry(
                            bytecode, &inst, env,
                        )?
                        .into(),
                    );

                    return Ok(Either::A(Self::fix_initial_modifier_inner(
                        trace.as_ref(),
                        stacktrace,
                        env,
                    )?));
                }
            }
        }

        Ok(Either::B(()))
    }

    /// Check if the last call/create that was done failed.
    pub fn check_failed_last_call_inner(
        trace: Either<&CallMessageTrace, &CreateMessageTrace>,
        stacktrace: SolidityStackTrace,
        env: Env,
    ) -> napi::Result<ModifiedStackTrace> {
        let (bytecode, steps) = match &trace {
            Either::A(call) => (&call.bytecode, &call.steps),
            Either::B(create) => (&create.bytecode, &create.steps),
        };

        let bytecode = bytecode.as_ref().expect("JS code asserts");

        for step_index in (0..steps.len() - 1).rev() {
            let (step, next_step) = match &steps[step_index..][..2] {
                &[Either4::A(ref step), ref next_step] => (step, next_step),
                _ => return Ok(ModifiedStackTrace::No(stacktrace)),
            };

            let inst = bytecode.get_instruction_inner(step.pc)?;
            let inst = inst.borrow(env)?;

            if let (Opcode::CALL | Opcode::CREATE, Either4::A(EvmStep { .. })) =
                (inst.opcode, next_step)
            {
                if Self::is_call_failed_error(trace, step_index as u32, &inst, env)? {
                    let mut stacktrace = stacktrace;
                    stacktrace.push(
                        Self::call_instruction_to_call_failed_to_execute_stack_trace_entry(
                            bytecode, &inst, env,
                        )?
                        .into(),
                    );

                    return Ok(ModifiedStackTrace::Yes(Self::fix_initial_modifier_inner(
                        trace, stacktrace, env,
                    )?));
                }
            }
        }

        Ok(ModifiedStackTrace::No(stacktrace))
    }

    /// Check if the trace reverted with a panic error.
    #[napi]
    pub fn check_panic(
        trace: Either<CallMessageTrace, CreateMessageTrace>,
        stacktrace: SolidityStackTrace,
        last_instruction: &Instruction,
        env: Env,
    ) -> napi::Result<Either<SolidityStackTrace, Undefined>> {
        Self::check_panic_inner(trace.as_ref(), stacktrace, last_instruction, env)
    }

    /// Check if the trace reverted with a panic error.
    pub fn check_panic_inner(
        trace: Either<&CallMessageTrace, &CreateMessageTrace>,
        mut stacktrace: SolidityStackTrace,
        last_instruction: &Instruction,
        env: Env,
    ) -> napi::Result<Either<SolidityStackTrace, Undefined>> {
        let return_data = ReturnData::new(
            match &trace {
                Either::A(call) => &call.return_data,
                Either::B(create) => &create.return_data,
            }
            .clone(),
        );

        if !return_data.is_panic_return_data() {
            return Ok(Either::B(()));
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
            Self::instruction_within_function_to_panic_stack_trace_entry_inner(
                trace,
                last_instruction,
                error_code,
                env,
            )?
            .into(),
        );

        Self::fix_initial_modifier_inner(trace, stacktrace, env).map(Either::A)
    }

    /// Check if the trace reverted with a panic error.
    pub fn check_panic_inner_modified_opt(
        trace: Either<&CallMessageTrace, &CreateMessageTrace>,
        mut stacktrace: SolidityStackTrace,
        last_instruction: &Instruction,
        env: Env,
    ) -> napi::Result<ModifiedStackTrace> {
        let return_data = ReturnData::new(
            match &trace {
                Either::A(call) => &call.return_data,
                Either::B(create) => &create.return_data,
            }
            .clone(),
        );

        if !return_data.is_panic_return_data() {
            return Ok(ModifiedStackTrace::No(stacktrace));
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
            Self::instruction_within_function_to_panic_stack_trace_entry_inner(
                trace,
                last_instruction,
                error_code,
                env,
            )?
            .into(),
        );

        Self::fix_initial_modifier_inner(trace, stacktrace, env).map(ModifiedStackTrace::Yes)
    }

    #[napi]
    pub fn check_custom_errors(
        trace: Either<CallMessageTrace, CreateMessageTrace>,
        stacktrace: SolidityStackTrace,
        last_instruction: &Instruction,
        env: Env,
    ) -> napi::Result<Either<SolidityStackTrace, Undefined>> {
        Self::check_custom_errors_inner(trace.as_ref(), stacktrace, last_instruction, env)
    }

    pub fn check_custom_errors_inner(
        trace: Either<&CallMessageTrace, &CreateMessageTrace>,
        stacktrace: SolidityStackTrace,
        last_instruction: &Instruction,
        env: Env,
    ) -> napi::Result<Either<SolidityStackTrace, Undefined>> {
        let (bytecode, return_data) = match &trace {
            Either::A(call) => (&call.bytecode, &call.return_data),
            Either::B(create) => (&create.bytecode, &create.return_data),
        };

        let bytecode = bytecode.as_ref().expect("JS code asserts");
        let contract = bytecode.contract.borrow(env)?;

        let return_data = ReturnData::new(return_data.clone());

        if return_data.is_empty() || return_data.is_error_return_data() {
            // if there is no return data, or if it's a Error(string),
            // then it can't be a custom error
            return Ok(Either::B(()));
        }

        let raw_return_data = hex::encode(&*return_data.value);
        let mut error_message = format!(
            "reverted with an unrecognized custom error (return data: 0x{raw_return_data})",
        );

        for custom_error in &contract.custom_errors {
            let custom_error = custom_error.borrow(env)?;

            if return_data.matches_selector(&*custom_error.selector) {
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
        drop(contract);

        let mut stacktrace = stacktrace;
        stacktrace.push(
            Self::instruction_within_function_to_custom_error_stack_trace_entry_inner(
                trace,
                last_instruction,
                error_message,
                env,
            )?
            .into(),
        );

        Self::fix_initial_modifier_inner(trace, stacktrace, env).map(Either::A)
    }

    pub fn check_custom_errors_inner_modified_opt(
        trace: Either<&CallMessageTrace, &CreateMessageTrace>,
        stacktrace: SolidityStackTrace,
        last_instruction: &Instruction,
        env: Env,
    ) -> napi::Result<ModifiedStackTrace> {
        let (bytecode, return_data) = match &trace {
            Either::A(call) => (&call.bytecode, &call.return_data),
            Either::B(create) => (&create.bytecode, &create.return_data),
        };

        let bytecode = bytecode.as_ref().expect("JS code asserts");
        let contract = bytecode.contract.borrow(env)?;

        let return_data = ReturnData::new(return_data.clone());

        if return_data.is_empty() || return_data.is_error_return_data() {
            // if there is no return data, or if it's a Error(string),
            // then it can't be a custom error
            return Ok(ModifiedStackTrace::No(stacktrace));
        }

        let raw_return_data = hex::encode(&*return_data.value);
        let mut error_message = format!(
            "reverted with an unrecognized custom error (return data: 0x{raw_return_data})",
        );

        for custom_error in &contract.custom_errors {
            let custom_error = custom_error.borrow(env)?;

            if return_data.matches_selector(&*custom_error.selector) {
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
        drop(contract);

        let mut stacktrace = stacktrace;
        stacktrace.push(
            Self::instruction_within_function_to_custom_error_stack_trace_entry_inner(
                trace,
                last_instruction,
                error_message,
                env,
            )?
            .into(),
        );

        Self::fix_initial_modifier_inner(trace, stacktrace, env).map(ModifiedStackTrace::Yes)
    }

    #[napi]
    pub fn check_solidity_0_6_3_unmapped_revert(
        trace: Either<CallMessageTrace, CreateMessageTrace>,
        mut stacktrace: SolidityStackTrace,
        env: Env,
    ) -> napi::Result<Either<SolidityStackTrace, Undefined>> {
        if Self::solidity_0_6_3_maybe_unmapped_revert_inner(trace.as_ref(), env)? {
            let revert_frame =
                Self::solidity_0_6_3_get_frame_for_unmapped_revert_within_function(trace, env)?;

            if let Either::A(revert_frame) = revert_frame {
                stacktrace.push(revert_frame.into());

                return Ok(Either::A(stacktrace));
            }

            return Ok(Either::A(stacktrace));
        }

        Ok(Either::B(()))
    }

    pub fn check_solidity_0_6_3_unmapped_revert_inner(
        trace: Either<&CallMessageTrace, &CreateMessageTrace>,
        mut stacktrace: SolidityStackTrace,
        env: Env,
    ) -> napi::Result<ModifiedStackTrace> {
        if Self::solidity_0_6_3_maybe_unmapped_revert_inner(trace, env)? {
            let revert_frame =
                Self::solidity_0_6_3_get_frame_for_unmapped_revert_within_function_inner(
                    trace, env,
                )?;

            if let Either::A(revert_frame) = revert_frame {
                stacktrace.push(revert_frame.into());

                return Ok(ModifiedStackTrace::Yes(stacktrace));
            }

            return Ok(ModifiedStackTrace::Yes(stacktrace));
        }

        Ok(ModifiedStackTrace::No(stacktrace))
    }

    #[napi(catch_unwind)]
    pub fn check_non_contract_called(
        trace: Either<CallMessageTrace, CreateMessageTrace>,
        mut stacktrace: SolidityStackTrace,
        env: Env,
    ) -> napi::Result<Either<SolidityStackTrace, Undefined>> {
        if Self::is_called_non_contract_account_error(trace.as_ref(), env)? {
            let source_reference = Self::get_last_source_reference_inner(trace.as_ref(), env)?;

            // We are sure this is not undefined because there was at least a call instruction
            let source_reference = match source_reference {
                Either::A(source_reference) => source_reference,
                Either::B(_) => panic!("Expected source reference to be defined"),
            };

            let non_contract_called_frame = NonContractAccountCalledErrorStackTraceEntry {
                type_: StackTraceEntryTypeConst,
                source_reference,
            };

            stacktrace.push(non_contract_called_frame.into());

            Ok(Either::A(stacktrace))
        } else {
            Ok(Either::B(()))
        }
    }

    pub fn check_non_contract_called_inner(
        trace: Either<&CallMessageTrace, &CreateMessageTrace>,
        mut stacktrace: SolidityStackTrace,
        env: Env,
    ) -> napi::Result<ModifiedStackTrace> {
        if Self::is_called_non_contract_account_error(trace, env)? {
            let source_reference = Self::get_last_source_reference_inner(trace, env)?;

            // We are sure this is not undefined because there was at least a call instruction
            let source_reference = match source_reference {
                Either::A(source_reference) => source_reference,
                Either::B(_) => panic!("Expected source reference to be defined"),
            };

            let non_contract_called_frame = NonContractAccountCalledErrorStackTraceEntry {
                type_: StackTraceEntryTypeConst,
                source_reference,
            };

            stacktrace.push(non_contract_called_frame.into());

            Ok(ModifiedStackTrace::Yes(stacktrace))
        } else {
            Ok(ModifiedStackTrace::No(stacktrace))
        }
    }

    /// Check if the last submessage can be used to generate the stack trace.
    #[napi]
    pub fn check_last_submessage(
        trace: Either<CallMessageTrace, CreateMessageTrace>,
        mut stacktrace: SolidityStackTrace,
        last_submessage_data: Either<SubmessageData, Undefined>,
        env: Env,
    ) -> napi::Result<Either<SolidityStackTrace, Undefined>> {
        let (bytecode, steps) = match &trace {
            Either::A(call) => (&call.bytecode, &call.steps),
            Either::B(create) => (&create.bytecode, &create.steps),
        };

        let bytecode = bytecode.as_ref().expect("JS code asserts");

        let last_submessage_data = match last_submessage_data {
            Either::A(last_submessage_data) => last_submessage_data,
            Either::B(()) => return Ok(Either::B(())),
        };

        // get the instruction before the submessage and add it to the stack trace
        let call_step = match steps.get(last_submessage_data.step_index as usize - 1) {
            Some(Either4::A(call_step)) => call_step,
            _ => panic!("This should not happen: MessageTrace should be preceded by a EVM step"),
        };

        let call_inst = bytecode.get_instruction_inner(call_step.pc)?;
        let call_inst = call_inst.borrow(env)?;
        let call_stack_frame =
            instruction_to_callstack_stack_trace_entry(bytecode, &call_inst, env)?;

        let (call_stack_frame_source_reference, call_stack_frame) = match call_stack_frame {
            Either::A(frame) => (frame.source_reference.clone(), frame.into()),
            Either::B(frame) => (frame.source_reference.clone(), frame.into()),
        };

        let last_message_failed = match last_submessage_data.message_trace {
            Either3::A(ref precompile) => precompile.exit.is_error(),
            Either3::B(ref call) => call.exit.is_error(),
            Either3::C(ref create) => create.exit.is_error(),
        };
        if last_message_failed {
            // add the call/create that generated the message to the stack trace

            stacktrace.push(call_stack_frame);

            if Self::is_subtrace_error_propagated_inner(
                trace.as_ref(),
                last_submessage_data.step_index,
                env,
            )? || Self::is_proxy_error_propagated_inner(
                trace.as_ref(),
                last_submessage_data.step_index,
                env,
            )? {
                stacktrace.extend(last_submessage_data.stacktrace);

                if Self::is_contract_call_run_out_of_gas_error_inner(
                    trace.as_ref(),
                    last_submessage_data.step_index,
                    env,
                )? {
                    let last_frame = match stacktrace.pop() {
                        Some(frame) => frame,
                        _ => panic!("Expected inferred stack trace to have at least one frame"),
                    };

                    stacktrace.push(
                        ContractCallRunOutOfGasError {
                            type_: StackTraceEntryTypeConst,
                            source_reference: last_frame.source_reference().cloned(),
                        }
                        .into(),
                    );
                }

                return Self::fix_initial_modifier_inner(trace.as_ref(), stacktrace, env)
                    .map(Either::A);
            }
        } else {
            let is_return_data_size_error = Self::fails_right_after_call_inner(
                trace.as_ref(),
                last_submessage_data.step_index,
                env,
            )?;
            if is_return_data_size_error {
                stacktrace.push(
                    ReturndataSizeErrorStackTraceEntry {
                        type_: StackTraceEntryTypeConst,
                        source_reference: call_stack_frame_source_reference,
                    }
                    .into(),
                );

                return Self::fix_initial_modifier_inner(trace.as_ref(), stacktrace, env)
                    .map(Either::A);
            }
        }

        Ok(Either::B(()))
    }

    /// Check if the last submessage can be used to generate the stack trace.
    fn check_last_submessage_inner(
        trace: Either<&CallMessageTrace, &CreateMessageTrace>,
        mut stacktrace: SolidityStackTrace,
        last_submessage_data: Either<SubmessageData, Undefined>,
        env: Env,
    ) -> napi::Result<ModifiedStackTrace> {
        let (bytecode, steps) = match &trace {
            Either::A(call) => (&call.bytecode, &call.steps),
            Either::B(create) => (&create.bytecode, &create.steps),
        };

        let bytecode = bytecode.as_ref().expect("JS code asserts");

        let last_submessage_data = match last_submessage_data {
            Either::A(last_submessage_data) => last_submessage_data,
            Either::B(()) => return Ok(ModifiedStackTrace::No(stacktrace)),
        };

        // get the instruction before the submessage and add it to the stack trace
        let call_step = match steps.get(last_submessage_data.step_index as usize - 1) {
            Some(Either4::A(call_step)) => call_step,
            _ => panic!("This should not happen: MessageTrace should be preceded by a EVM step"),
        };

        let call_inst = bytecode.get_instruction_inner(call_step.pc)?;
        let call_inst = call_inst.borrow(env)?;
        let call_stack_frame =
            instruction_to_callstack_stack_trace_entry(bytecode, &call_inst, env)?;

        let (call_stack_frame_source_reference, call_stack_frame) = match call_stack_frame {
            Either::A(frame) => (frame.source_reference.clone(), frame.into()),
            Either::B(frame) => (frame.source_reference.clone(), frame.into()),
        };

        let last_message_failed = match last_submessage_data.message_trace {
            Either3::A(ref precompile) => precompile.exit.is_error(),
            Either3::B(ref call) => call.exit.is_error(),
            Either3::C(ref create) => create.exit.is_error(),
        };
        if last_message_failed {
            // add the call/create that generated the message to the stack trace

            stacktrace.push(call_stack_frame);

            if Self::is_subtrace_error_propagated_inner(
                trace,
                last_submessage_data.step_index,
                env,
            )? || Self::is_proxy_error_propagated_inner(
                trace,
                last_submessage_data.step_index,
                env,
            )? {
                stacktrace.extend(last_submessage_data.stacktrace);

                if Self::is_contract_call_run_out_of_gas_error_inner(
                    trace,
                    last_submessage_data.step_index,
                    env,
                )? {
                    let last_frame = match stacktrace.pop() {
                        Some(frame) => frame,
                        _ => panic!("Expected inferred stack trace to have at least one frame"),
                    };

                    stacktrace.push(
                        ContractCallRunOutOfGasError {
                            type_: StackTraceEntryTypeConst,
                            source_reference: last_frame.source_reference().cloned(),
                        }
                        .into(),
                    );
                }

                return Self::fix_initial_modifier_inner(trace, stacktrace, env)
                    .map(ModifiedStackTrace::Yes);
            }
        } else {
            let is_return_data_size_error =
                Self::fails_right_after_call_inner(trace, last_submessage_data.step_index, env)?;
            if is_return_data_size_error {
                stacktrace.push(
                    ReturndataSizeErrorStackTraceEntry {
                        type_: StackTraceEntryTypeConst,
                        source_reference: call_stack_frame_source_reference,
                    }
                    .into(),
                );

                return Self::fix_initial_modifier_inner(trace, stacktrace, env)
                    .map(ModifiedStackTrace::Yes);
            }
        }

        Ok(ModifiedStackTrace::No(stacktrace))
    }

    /// Check if the execution stopped with a revert or an invalid opcode.
    #[napi]
    pub fn check_revert_or_invalid_opcode(
        trace: Either<CallMessageTrace, CreateMessageTrace>,
        stacktrace: SolidityStackTrace,
        last_instruction: &Instruction,
        function_jumpdests: Vec<&Instruction>,
        jumped_into_function: bool,
        env: Env,
    ) -> napi::Result<Either<SolidityStackTrace, Undefined>> {
        match last_instruction.opcode {
            Opcode::REVERT | Opcode::INVALID => {}
            _ => return Ok(Either::B(())),
        }

        let (bytecode, return_data) = match &trace {
            Either::A(call) => (&call.bytecode, &call.return_data),
            Either::B(create) => (&create.bytecode, &create.return_data),
        };
        let bytecode = bytecode.as_ref().expect("JS code asserts");

        let mut inferred_stacktrace = stacktrace;

        if let Some(location) = &last_instruction.location {
            if jumped_into_function || matches!(trace, Either::B(CreateMessageTrace { .. })) {
                // There should always be a function here, but that's not the case with optimizations.
                //
                // If this is a create trace, we already checked args and nonpayable failures before
                // calling this function.
                //
                // If it's a call trace, we already jumped into a function. But optimizations can happen.
                let location = location.borrow(env)?;
                let failing_function = location.get_containing_function_inner(env)?;

                // If the failure is in a modifier we add an entry with the function/constructor
                match failing_function.into_option() {
                    Some(func) if func.borrow(env)?.r#type == ContractFunctionType::MODIFIER => {
                        let frame = Self::get_entry_before_failure_in_modifier_inner(
                            trace.as_ref(),
                            function_jumpdests,
                            env,
                        )?;

                        inferred_stacktrace.push(match frame {
                            Either::A(frame) => frame.into(),
                            Either::B(frame) => frame.into(),
                        });
                    }
                    _ => {}
                }
            }
        }

        let panic_stacktrace = Self::check_panic_inner_modified_opt(
            trace.as_ref(),
            inferred_stacktrace,
            last_instruction,
            env,
        )?;
        let inferred_stacktrace = match panic_stacktrace {
            ModifiedStackTrace::Yes(stacktrace) => return Ok(Either::A(stacktrace)),
            ModifiedStackTrace::No(stacktrace) => stacktrace,
        };

        let custom_error_stacktrace = Self::check_custom_errors_inner_modified_opt(
            trace.as_ref(),
            inferred_stacktrace,
            last_instruction,
            env,
        )?;
        let mut inferred_stacktrace = match custom_error_stacktrace {
            ModifiedStackTrace::Yes(stacktrace) => return Ok(Either::A(stacktrace)),
            ModifiedStackTrace::No(stacktrace) => stacktrace,
        };

        if let Some(location) = &last_instruction.location {
            if jumped_into_function || matches!(trace, Either::B(CreateMessageTrace { .. })) {
                let location = location.borrow(env)?;
                let failing_function = location.get_containing_function_inner(env)?;

                if failing_function.into_option().is_some() {
                    let frame =
                        Self::instruction_within_function_to_revert_stack_trace_entry_inner(
                            trace.as_ref(),
                            last_instruction,
                            env,
                        )?;

                    inferred_stacktrace.push(frame.into());
                } else {
                    let message = ReturnData::new(return_data.clone()).into_instance(env)?;
                    let is_invalid_opcode_error = last_instruction.opcode == Opcode::INVALID;

                    match &trace {
                        Either::A(CallMessageTrace { calldata, .. }) => {
                            let contract = bytecode.contract.borrow(env)?;

                            // This is here because of the optimizations
                            let function_selector = contract.get_function_from_selector_inner(
                                calldata.get(..4).unwrap_or(&calldata[..]),
                            );

                            // in general this shouldn't happen, but it does when viaIR is enabled,
                            // "optimizerSteps": "u" is used, and the called function is fallback or
                            // receive
                            let Some(function_selector) = function_selector else {
                                return Ok(Either::B(()));
                            };

                            let function = function_selector.borrow(env)?;

                            let frame = RevertErrorStackTraceEntry {
                                type_: StackTraceEntryTypeConst,
                                source_reference: Self::get_function_start_source_reference_inner(
                                    trace.as_ref(),
                                    &function,
                                    env,
                                )?,
                                message,
                                is_invalid_opcode_error,
                            };

                            inferred_stacktrace.push(frame.into());
                        }
                        Either::B(trace @ CreateMessageTrace { .. }) => {
                            // This is here because of the optimizations
                            let frame = RevertErrorStackTraceEntry {
                                type_: StackTraceEntryTypeConst,
                                source_reference:
                                    Self::get_constructor_start_source_reference_inner(trace, env)?,
                                message,
                                is_invalid_opcode_error,
                            };

                            inferred_stacktrace.push(frame.into());
                        }
                    }
                }

                return Self::fix_initial_modifier_inner(trace.as_ref(), inferred_stacktrace, env)
                    .map(Either::A);
            }
        }

        // If the revert instruction is not mapped but there is return data,
        // we add the frame anyway, sith the best sourceReference we can get
        if last_instruction.location.is_none() && !return_data.is_empty() {
            let revert_frame = RevertErrorStackTraceEntry {
                type_: StackTraceEntryTypeConst,
                source_reference: Self::get_contract_start_without_function_source_reference_inner(
                    trace.as_ref(),
                    env,
                )?,
                message: ReturnData::new(return_data.clone()).into_instance(env)?,
                is_invalid_opcode_error: last_instruction.opcode == Opcode::INVALID,
            };

            inferred_stacktrace.push(revert_frame.into());

            return Self::fix_initial_modifier_inner(trace.as_ref(), inferred_stacktrace, env)
                .map(Either::A);
        }

        Ok(Either::B(()))
    }

    /// Check if the execution stopped with a revert or an invalid opcode.
    pub fn check_revert_or_invalid_opcode_modified_opt(
        trace: Either<&CallMessageTrace, &CreateMessageTrace>,
        stacktrace: SolidityStackTrace,
        last_instruction: &Instruction,
        function_jumpdests: Vec<&Instruction>,
        jumped_into_function: bool,
        env: Env,
    ) -> napi::Result<ModifiedStackTrace> {
        match last_instruction.opcode {
            Opcode::REVERT | Opcode::INVALID => {}
            _ => return Ok(ModifiedStackTrace::No(stacktrace)),
        }

        let (bytecode, return_data) = match &trace {
            Either::A(call) => (&call.bytecode, &call.return_data),
            Either::B(create) => (&create.bytecode, &create.return_data),
        };
        let bytecode = bytecode.as_ref().expect("JS code asserts");

        let mut inferred_stacktrace = stacktrace;

        if let Some(location) = &last_instruction.location {
            if jumped_into_function || matches!(trace, Either::B(CreateMessageTrace { .. })) {
                // There should always be a function here, but that's not the case with optimizations.
                //
                // If this is a create trace, we already checked args and nonpayable failures before
                // calling this function.
                //
                // If it's a call trace, we already jumped into a function. But optimizations can happen.
                let location = location.borrow(env)?;
                let failing_function = location.get_containing_function_inner(env)?;

                // If the failure is in a modifier we add an entry with the function/constructor
                match failing_function.into_option() {
                    Some(func) if func.borrow(env)?.r#type == ContractFunctionType::MODIFIER => {
                        let frame = Self::get_entry_before_failure_in_modifier_inner(
                            trace,
                            function_jumpdests,
                            env,
                        )?;

                        inferred_stacktrace.push(match frame {
                            Either::A(frame) => frame.into(),
                            Either::B(frame) => frame.into(),
                        });
                    }
                    _ => {}
                }
            }
        }

        let panic_stacktrace = Self::check_panic_inner_modified_opt(
            trace,
            inferred_stacktrace,
            last_instruction,
            env,
        )?;
        let inferred_stacktrace = match panic_stacktrace {
            yes @ ModifiedStackTrace::Yes(..) => return Ok(yes),
            ModifiedStackTrace::No(stacktrace) => stacktrace,
        };

        let custom_error_stacktrace = Self::check_custom_errors_inner_modified_opt(
            trace,
            inferred_stacktrace,
            last_instruction,
            env,
        )?;
        let mut inferred_stacktrace = match custom_error_stacktrace {
            yes @ ModifiedStackTrace::Yes(..) => return Ok(yes),
            ModifiedStackTrace::No(stacktrace) => stacktrace,
        };

        if let Some(location) = &last_instruction.location {
            if jumped_into_function || matches!(trace, Either::B(CreateMessageTrace { .. })) {
                let location = location.borrow(env)?;
                let failing_function = location.get_containing_function_inner(env)?;

                if failing_function.into_option().is_some() {
                    let frame =
                        Self::instruction_within_function_to_revert_stack_trace_entry_inner(
                            trace,
                            last_instruction,
                            env,
                        )?;

                    inferred_stacktrace.push(frame.into());
                } else {
                    let message = ReturnData::new(return_data.clone()).into_instance(env)?;
                    let is_invalid_opcode_error = last_instruction.opcode == Opcode::INVALID;

                    match &trace {
                        Either::A(CallMessageTrace { calldata, .. }) => {
                            let contract = bytecode.contract.borrow(env)?;

                            // This is here because of the optimizations
                            let function_selector = contract.get_function_from_selector_inner(
                                calldata.get(..4).unwrap_or(&calldata[..]),
                            );

                            // in general this shouldn't happen, but it does when viaIR is enabled,
                            // "optimizerSteps": "u" is used, and the called function is fallback or
                            // receive
                            let Some(function_selector) = function_selector else {
                                return Ok(ModifiedStackTrace::No(inferred_stacktrace));
                            };

                            let function = function_selector.borrow(env)?;

                            let frame = RevertErrorStackTraceEntry {
                                type_: StackTraceEntryTypeConst,
                                source_reference: Self::get_function_start_source_reference_inner(
                                    trace, &function, env,
                                )?,
                                message,
                                is_invalid_opcode_error,
                            };

                            inferred_stacktrace.push(frame.into());
                        }
                        Either::B(trace @ CreateMessageTrace { .. }) => {
                            // This is here because of the optimizations
                            let frame = RevertErrorStackTraceEntry {
                                type_: StackTraceEntryTypeConst,
                                source_reference:
                                    Self::get_constructor_start_source_reference_inner(trace, env)?,
                                message,
                                is_invalid_opcode_error,
                            };

                            inferred_stacktrace.push(frame.into());
                        }
                    }
                }

                return Self::fix_initial_modifier_inner(trace, inferred_stacktrace, env)
                    .map(ModifiedStackTrace::Yes);
            }
        }

        // If the revert instruction is not mapped but there is return data,
        // we add the frame anyway, sith the best sourceReference we can get
        if last_instruction.location.is_none() && !return_data.is_empty() {
            let revert_frame = RevertErrorStackTraceEntry {
                type_: StackTraceEntryTypeConst,
                source_reference: Self::get_contract_start_without_function_source_reference_inner(
                    trace, env,
                )?,
                message: ReturnData::new(return_data.clone()).into_instance(env)?,
                is_invalid_opcode_error: last_instruction.opcode == Opcode::INVALID,
            };

            inferred_stacktrace.push(revert_frame.into());

            return Self::fix_initial_modifier_inner(trace, inferred_stacktrace, env)
                .map(ModifiedStackTrace::Yes);
        }

        Ok(ModifiedStackTrace::No(inferred_stacktrace))
    }

    /// Check last instruction to try to infer the error.
    #[napi]
    pub fn check_last_instruction(
        trace: Either<CallMessageTrace, CreateMessageTrace>,
        stacktrace: SolidityStackTrace,
        function_jumpdests: Vec<&Instruction>,
        jumped_into_function: bool,
        env: Env,
    ) -> napi::Result<Either<SolidityStackTrace, Undefined>> {
        let (bytecode, steps) = match &trace {
            Either::A(call) => (&call.bytecode, &call.steps),
            Either::B(create) => (&create.bytecode, &create.steps),
        };
        let bytecode = bytecode.as_ref().expect("JS code asserts");

        if steps.is_empty() {
            return Ok(Either::B(()));
        }

        let last_step = match steps.last() {
            Some(Either4::A(step)) => step,
            _ => panic!("This should not happen: MessageTrace ends with a subtrace"),
        };

        let last_instruction = bytecode.get_instruction_inner(last_step.pc)?;

        let revert_or_invalid_stacktrace = Self::check_revert_or_invalid_opcode_modified_opt(
            trace.as_ref(),
            stacktrace,
            &*last_instruction.borrow(env)?,
            function_jumpdests,
            jumped_into_function,
            env,
        )?;
        if let ModifiedStackTrace::Yes(stacktrace) = revert_or_invalid_stacktrace {
            return Ok(Either::A(stacktrace));
        }

        let (Either::A(trace @ CallMessageTrace { ref calldata, .. }), false) =
            (&trace, jumped_into_function)
        else {
            return Ok(Either::B(()));
        };

        let last_instruction = last_instruction.borrow(env)?;

        if Self::has_failed_inside_the_fallback_function_inner(trace, env)?
            || Self::has_failed_inside_the_receive_function_inner(trace, env)?
        {
            let frame = Self::instruction_within_function_to_revert_stack_trace_entry_inner(
                Either::A(trace),
                &last_instruction,
                env,
            )?;

            return Ok(Either::A(vec![frame.into()]));
        }

        // Sometimes we do fail inside of a function but there's no jump into
        if let Some(location) = &last_instruction.location {
            let location = location.borrow(env)?;
            let failing_function = location.get_containing_function_inner(env)?;

            if let Some(failing_function) = failing_function.into_option() {
                let failing_function = failing_function.borrow(env)?;

                let frame = RevertErrorStackTraceEntry {
                    type_: StackTraceEntryTypeConst,
                    source_reference: Self::get_function_start_source_reference_inner(
                        Either::A(trace),
                        &failing_function,
                        env,
                    )?,
                    message: ReturnData::new(trace.return_data.clone()).into_instance(env)?,
                    is_invalid_opcode_error: last_instruction.opcode == Opcode::INVALID,
                };

                return Ok(Either::A(vec![frame.into()]));
            }
        }

        let contract = bytecode.contract.borrow(env)?;

        let selector = calldata.get(..4).unwrap_or(&calldata[..]);
        let calldata = &calldata.get(4..).unwrap_or(&[]);

        let called_function = contract.get_function_from_selector_inner(selector);

        if let Some(called_function) = called_function {
            let called_function = called_function.borrow(env)?;

            let abi = called_function.to_alloy().map_err(|e| {
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
                    source_reference: Self::get_function_start_source_reference_inner(
                        Either::A(trace),
                        &called_function,
                        env,
                    )?,
                };

                return Ok(Either::A(vec![frame.into()]));
            }
        }

        if Self::solidity_0_6_3_maybe_unmapped_revert_inner(Either::A(trace), env)? {
            let revert_frame =
                Self::solidity_0_6_3_get_frame_for_unmapped_revert_before_function_inner(
                    trace, env,
                )?;

            if let Some(revert_frame) = revert_frame.into_option() {
                return Ok(Either::A(vec![revert_frame.into()]));
            }
        }

        let frame =
            Self::get_other_error_before_called_function_stack_trace_entry_inner(trace, env)?;

        Ok(Either::A(vec![frame.into()]))
    }

    pub fn check_last_instruction_inner(
        trace: Either<&CallMessageTrace, &CreateMessageTrace>,
        stacktrace: SolidityStackTrace,
        function_jumpdests: Vec<&Instruction>,
        jumped_into_function: bool,
        env: Env,
    ) -> napi::Result<ModifiedStackTrace> {
        let (bytecode, steps) = match &trace {
            Either::A(call) => (&call.bytecode, &call.steps),
            Either::B(create) => (&create.bytecode, &create.steps),
        };
        let bytecode = bytecode.as_ref().expect("JS code asserts");

        if steps.is_empty() {
            return Ok(ModifiedStackTrace::No(stacktrace));
        }

        let last_step = match steps.last() {
            Some(Either4::A(step)) => step,
            _ => panic!("This should not happen: MessageTrace ends with a subtrace"),
        };

        let last_instruction = bytecode.get_instruction_inner(last_step.pc)?;

        let revert_or_invalid_stacktrace = Self::check_revert_or_invalid_opcode_modified_opt(
            trace,
            stacktrace,
            &*last_instruction.borrow(env)?,
            function_jumpdests,
            jumped_into_function,
            env,
        )?;
        let stacktrace = match revert_or_invalid_stacktrace {
            yes @ ModifiedStackTrace::Yes(..) => return Ok(yes),
            ModifiedStackTrace::No(stacktrace) => stacktrace,
        };

        let (Either::A(trace @ CallMessageTrace { ref calldata, .. }), false) =
            (&trace, jumped_into_function)
        else {
            return Ok(ModifiedStackTrace::No(stacktrace));
        };

        let last_instruction = last_instruction.borrow(env)?;

        if Self::has_failed_inside_the_fallback_function_inner(trace, env)?
            || Self::has_failed_inside_the_receive_function_inner(trace, env)?
        {
            let frame = Self::instruction_within_function_to_revert_stack_trace_entry_inner(
                Either::A(trace),
                &last_instruction,
                env,
            )?;

            return Ok(ModifiedStackTrace::Yes(vec![frame.into()]));
        }

        // Sometimes we do fail inside of a function but there's no jump into
        if let Some(location) = &last_instruction.location {
            let location = location.borrow(env)?;
            let failing_function = location.get_containing_function_inner(env)?;

            if let Some(failing_function) = failing_function.into_option() {
                let failing_function = failing_function.borrow(env)?;

                let frame = RevertErrorStackTraceEntry {
                    type_: StackTraceEntryTypeConst,
                    source_reference: Self::get_function_start_source_reference_inner(
                        Either::A(trace),
                        &failing_function,
                        env,
                    )?,
                    message: ReturnData::new(trace.return_data.clone()).into_instance(env)?,
                    is_invalid_opcode_error: last_instruction.opcode == Opcode::INVALID,
                };

                return Ok(ModifiedStackTrace::Yes(vec![frame.into()]));
            }
        }

        let contract = bytecode.contract.borrow(env)?;

        let selector = calldata.get(..4).unwrap_or(&calldata[..]);
        let calldata = &calldata.get(4..).unwrap_or(&[]);

        let called_function = contract.get_function_from_selector_inner(selector);

        if let Some(called_function) = called_function {
            let called_function = called_function.borrow(env)?;

            let abi = called_function.to_alloy().map_err(|e| {
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
                    source_reference: Self::get_function_start_source_reference_inner(
                        Either::A(trace),
                        &called_function,
                        env,
                    )?,
                };

                return Ok(ModifiedStackTrace::Yes(vec![frame.into()]));
            }
        }

        if Self::solidity_0_6_3_maybe_unmapped_revert_inner(Either::A(trace), env)? {
            let revert_frame =
                Self::solidity_0_6_3_get_frame_for_unmapped_revert_before_function_inner(
                    trace, env,
                )?;

            if let Some(revert_frame) = revert_frame.into_option() {
                return Ok(ModifiedStackTrace::Yes(vec![revert_frame.into()]));
            }
        }

        let frame =
            Self::get_other_error_before_called_function_stack_trace_entry_inner(trace, env)?;

        Ok(ModifiedStackTrace::Yes(vec![frame.into()]))
    }

    // Helpers

    #[napi]
    pub fn fix_initial_modifier(
        trace: Either<CallMessageTrace, CreateMessageTrace>,
        stacktrace: SolidityStackTrace,
        env: Env,
    ) -> napi::Result<SolidityStackTrace> {
        Self::fix_initial_modifier_inner(trace.as_ref(), stacktrace, env)
    }

    fn fix_initial_modifier_inner(
        trace: Either<&CallMessageTrace, &CreateMessageTrace>,
        mut stacktrace: SolidityStackTrace,
        env: Env,
    ) -> napi::Result<SolidityStackTrace> {
        if let Some(Either24::A(CallstackEntryStackTraceEntry {
            function_type: ContractFunctionType::MODIFIER,
            ..
        })) = stacktrace.first()
        {
            let entry_before_initial_modifier =
                Self::get_entry_before_initial_modifier_callstack_entry(trace, env)?;

            stacktrace.insert(0, entry_before_initial_modifier);
        }

        Ok(stacktrace)
    }

    fn get_entry_before_initial_modifier_callstack_entry(
        trace: Either<&CallMessageTrace, &CreateMessageTrace>,
        env: Env,
    ) -> napi::Result<SolidityStackTraceEntry> {
        let trace = match trace {
            Either::B(create) => {
                return Ok(CallstackEntryStackTraceEntry {
                    type_: StackTraceEntryTypeConst,
                    source_reference: Self::get_constructor_start_source_reference_inner(
                        create, env,
                    )?,
                    function_type: ContractFunctionType::CONSTRUCTOR,
                }
                .into())
            }
            Either::A(call) => call,
        };

        let bytecode = trace.bytecode.as_ref().expect("JS code asserts");
        let contract = bytecode.contract.borrow(env)?;

        let called_function = contract.get_function_from_selector_inner(
            trace.calldata.get(..4).unwrap_or(&trace.calldata[..]),
        );

        let called_function = called_function.map(|x| x.borrow(env)).transpose()?;
        let called_function = called_function.as_deref();

        let source_reference = match called_function {
            Some(called_function) => Self::get_function_start_source_reference_inner(
                Either::A(trace),
                called_function,
                env,
            )?,
            None => Self::get_fallback_start_source_reference_inner(trace, env)?,
        };

        let function_type = match called_function {
            Some(_) => ContractFunctionType::FUNCTION,
            None => ContractFunctionType::FALLBACK,
        };

        Ok(CallstackEntryStackTraceEntry {
            type_: StackTraceEntryTypeConst,
            source_reference,
            function_type,
        }
        .into())
    }

    #[napi(catch_unwind)]
    pub fn call_instruction_to_call_failed_to_execute_stack_trace_entry(
        bytecode: &Bytecode,
        call_inst: &Instruction,
        env: Env,
    ) -> napi::Result<CallFailedErrorStackTraceEntry> {
        let location = call_inst
            .location
            .as_ref()
            .map(|l| l.borrow(env))
            .transpose()?;
        let location = location.as_deref();

        let source_reference = source_location_to_source_reference(bytecode, location, env)?;
        let source_reference = source_reference.expect("Expected source reference to be defined");

        // Calls only happen within functions
        Ok(CallFailedErrorStackTraceEntry {
            type_: StackTraceEntryTypeConst,
            source_reference,
        })
    }

    #[napi(catch_unwind)]
    pub fn get_entry_before_failure_in_modifier(
        trace: Either<CallMessageTrace, CreateMessageTrace>,
        function_jumpdests: Vec<&Instruction>,
        env: Env,
    ) -> napi::Result<Either<CallstackEntryStackTraceEntry, InternalFunctionCallStackEntry>> {
        Self::get_entry_before_failure_in_modifier_inner(trace.as_ref(), function_jumpdests, env)
    }

    pub fn get_entry_before_failure_in_modifier_inner(
        trace: Either<&CallMessageTrace, &CreateMessageTrace>,
        function_jumpdests: Vec<&Instruction>,
        env: Env,
    ) -> napi::Result<Either<CallstackEntryStackTraceEntry, InternalFunctionCallStackEntry>> {
        let bytecode = match &trace {
            Either::A(call) => &call.bytecode,
            Either::B(create) => &create.bytecode,
        };
        let bytecode = bytecode.as_ref().expect("JS code asserts");

        // If there's a jumpdest, this modifier belongs to the last function that it represents
        if let Some(last_jumpdest) = function_jumpdests.last() {
            let entry = instruction_to_callstack_stack_trace_entry(bytecode, last_jumpdest, env)?;

            return Ok(entry);
        }

        let trace = match trace {
            Either::A(_call) => unreachable!("This shouldn't happen: a call trace has no functionJumpdest but has already jumped into a function"),
            Either::B(create) => create,
        };

        // If there's no jump dest, we point to the constructor.
        Ok(Either::A(CallstackEntryStackTraceEntry {
            type_: StackTraceEntryTypeConst,
            source_reference: Self::get_constructor_start_source_reference_inner(trace, env)?,
            function_type: ContractFunctionType::CONSTRUCTOR,
        }))
    }

    #[napi]
    pub fn fails_right_after_call(
        trace: Either<CallMessageTrace, CreateMessageTrace>,
        call_subtrace_step_index: u32,
        env: Env,
    ) -> napi::Result<bool> {
        Self::fails_right_after_call_inner(trace.as_ref(), call_subtrace_step_index, env)
    }

    fn fails_right_after_call_inner(
        trace: Either<&CallMessageTrace, &CreateMessageTrace>,
        call_subtrace_step_index: u32,
        env: Env,
    ) -> napi::Result<bool> {
        let (bytecode, steps) = match &trace {
            Either::A(call) => (&call.bytecode, &call.steps),
            Either::B(create) => (&create.bytecode, &create.steps),
        };

        let bytecode = bytecode.as_ref().expect("JS code asserts");

        let Some(Either4::A(last_step)) = steps.last() else {
            return Ok(false);
        };

        let last_inst = bytecode.get_instruction_inner(last_step.pc)?;
        let last_inst = last_inst.borrow(env)?;
        if last_inst.opcode != Opcode::REVERT {
            return Ok(false);
        }

        let call_opcode_step = steps.get(call_subtrace_step_index as usize - 1);
        let call_opcode_step = match call_opcode_step {
            Some(Either4::A(step)) => step,
            _ => panic!("JS code asserts this is always an EvmStep"),
        };
        let call_inst = bytecode.get_instruction_inner(call_opcode_step.pc)?;
        let call_inst = call_inst.borrow(env)?;

        // Calls are always made from within functions
        let call_inst_location = call_inst
            .location
            .as_ref()
            .expect("Expected call instruction location to be defined");
        let call_inst_location = call_inst_location.borrow(env)?;

        Self::is_last_location(
            trace,
            call_subtrace_step_index + 1,
            &call_inst_location,
            env,
        )
    }

    fn is_call_failed_error(
        trace: Either<&CallMessageTrace, &CreateMessageTrace>,
        inst_index: u32,
        call_instruction: &Instruction,
        env: Env,
    ) -> napi::Result<bool> {
        let call_location = match &call_instruction.location {
            Some(location) => location.borrow(env)?,
            None => panic!("Expected call location to be defined"),
        };

        Self::is_last_location(trace, inst_index, &call_location, env)
    }

    fn is_last_location(
        trace: Either<&CallMessageTrace, &CreateMessageTrace>,
        from_step: u32,
        location: &SourceLocation,
        env: Env,
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

            let step_inst = bytecode.get_instruction_inner(step.pc)?;
            let step_inst = step_inst.borrow(env)?;

            if let Some(step_inst_location) = &step_inst.location {
                let step_inst_location = step_inst_location.borrow(env)?;

                if !step_inst_location.equals(location, env) {
                    return Ok(false);
                }
            }
        }

        Ok(true)
    }

    #[napi]
    pub fn is_subtrace_error_propagated(
        trace: Either<CallMessageTrace, CreateMessageTrace>,
        call_subtrace_step_index: u32,
        env: Env,
    ) -> napi::Result<bool> {
        Self::is_subtrace_error_propagated_inner(trace.as_ref(), call_subtrace_step_index, env)
    }

    pub fn is_subtrace_error_propagated_inner(
        trace: Either<&CallMessageTrace, &CreateMessageTrace>,
        call_subtrace_step_index: u32,
        env: Env,
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

        Self::fails_right_after_call_inner(trace, call_subtrace_step_index, env)
    }

    #[napi(catch_unwind)]
    pub fn is_contract_call_run_out_of_gas_error(
        trace: Either<CallMessageTrace, CreateMessageTrace>,
        call_step_index: u32,
        env: Env,
    ) -> napi::Result<bool> {
        Self::is_contract_call_run_out_of_gas_error_inner(trace.as_ref(), call_step_index, env)
    }

    pub fn is_contract_call_run_out_of_gas_error_inner(
        trace: Either<&CallMessageTrace, &CreateMessageTrace>,
        call_step_index: u32,
        env: Env,
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

        Self::fails_right_after_call_inner(trace, call_step_index, env)
    }

    #[napi]
    pub fn is_proxy_error_propagated(
        trace: Either<CallMessageTrace, CreateMessageTrace>,
        call_subtrace_step_index: u32,
        env: Env,
    ) -> napi::Result<bool> {
        Self::is_proxy_error_propagated_inner(trace.as_ref(), call_subtrace_step_index, env)
    }

    pub fn is_proxy_error_propagated_inner(
        trace: Either<&CallMessageTrace, &CreateMessageTrace>,
        call_subtrace_step_index: u32,
        env: Env,
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

        let call_inst = bytecode.get_instruction_inner(call_step.pc)?;
        let call_inst = call_inst.borrow(env)?;

        if call_inst.opcode != Opcode::DELEGATECALL {
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

        if subtrace_bytecode.contract.borrow(env)?.r#type == ContractType::LIBRARY {
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

            let inst = subtrace_bytecode.get_instruction_inner(step.pc)?;
            let inst = inst.borrow(env)?;

            // All the remaining locations should be valid, as they are part of the inline asm
            if inst.location.is_none() {
                return Ok(false);
            }

            if matches!(
                inst.jump_type,
                JumpType::INTO_FUNCTION | JumpType::OUTOF_FUNCTION
            ) {
                return Ok(false);
            }
        }

        let last_step = match trace.steps.last() {
            Some(Either4::A(step)) => step,
            _ => panic!("Expected last step to be an EvmStep"),
        };
        let last_inst = bytecode.get_instruction_inner(last_step.pc)?;

        Ok(last_inst.borrow(env)?.opcode == Opcode::REVERT)
    }

    #[napi]
    pub fn other_execution_error_stacktrace(
        trace: Either<CallMessageTrace, CreateMessageTrace>,
        mut stacktrace: SolidityStackTrace,
        env: Env,
    ) -> napi::Result<SolidityStackTrace> {
        let other_execution_error_frame = OtherExecutionErrorStackTraceEntry {
            type_: StackTraceEntryTypeConst,
            source_reference: match Self::get_last_source_reference(trace, env)? {
                Either::A(source_reference) => Some(source_reference),
                Either::B(_) => None,
            },
        };

        stacktrace.push(other_execution_error_frame.into());
        Ok(stacktrace)
    }

    pub fn other_execution_error_stacktrace_inner(
        trace: Either<&CallMessageTrace, &CreateMessageTrace>,
        mut stacktrace: SolidityStackTrace,
        env: Env,
    ) -> napi::Result<SolidityStackTrace> {
        let other_execution_error_frame = OtherExecutionErrorStackTraceEntry {
            type_: StackTraceEntryTypeConst,
            source_reference: match Self::get_last_source_reference_inner(trace, env)? {
                Either::A(source_reference) => Some(source_reference),
                Either::B(_) => None,
            },
        };

        stacktrace.push(other_execution_error_frame.into());
        Ok(stacktrace)
    }

    fn is_direct_library_call(trace: &CallMessageTrace, env: Env) -> napi::Result<bool> {
        let contract = &trace.bytecode.as_ref().expect("JS code asserts").contract;
        let contract = contract.borrow(env)?;

        Ok(trace.depth == 0 && contract.r#type == ContractType::LIBRARY)
    }

    fn get_direct_library_call_error_stack_trace(
        trace: &CallMessageTrace,
        env: Env,
    ) -> napi::Result<SolidityStackTrace> {
        let contract = &trace.bytecode.as_ref().expect("JS code asserts").contract;
        let contract = contract.borrow(env)?;

        let func = contract.get_function_from_selector_inner(
            trace.calldata.get(..4).unwrap_or(&trace.calldata[..]),
        );

        let func = func.map(|f| f.borrow(env)).transpose()?;
        let source_reference = match func {
            Some(func) => {
                Self::get_function_start_source_reference_inner(Either::A(trace), &func, env)?
            }
            None => Self::get_contract_start_without_function_source_reference_inner(
                Either::A(trace),
                env,
            )?,
        };

        Ok(vec![DirectLibraryCallErrorStackTraceEntry {
            type_: StackTraceEntryTypeConst,
            source_reference,
        }
        .into()])
    }

    #[napi]
    pub fn get_function_start_source_reference(
        trace: Either<CallMessageTrace, CreateMessageTrace>,
        func: &ContractFunction,
        env: Env,
    ) -> napi::Result<SourceReference> {
        let trace = match &trace {
            Either::A(call) => Either::A(call),
            Either::B(create) => Either::B(create),
        };
        Self::get_function_start_source_reference_inner(trace, func, env)
    }

    fn get_function_start_source_reference_inner(
        trace: Either<&CallMessageTrace, &CreateMessageTrace>,
        func: &ContractFunction,
        env: Env,
    ) -> napi::Result<SourceReference> {
        let bytecode = match &trace {
            Either::A(create) => &create.bytecode,
            Either::B(call) => &call.bytecode,
        };

        let contract = &bytecode.as_ref().expect("JS code asserts").contract;
        let contract = contract.borrow(env)?;

        let location = func.location.borrow(env)?;
        let file = location.file.borrow(env)?;

        let location = func.location.borrow(env)?;

        Ok(SourceReference {
            source_name: file.source_name.clone(),
            source_content: file.content.clone(),
            contract: Some(contract.name.clone()),

            function: Some(func.name.clone()),
            line: location.get_starting_line_number(env).unwrap(),
            range: [location.offset, location.offset + location.length].to_vec(),
        })
    }

    fn is_missing_function_and_fallback_error(
        trace: &CallMessageTrace,
        called_function: Option<&ContractFunction>,
        env: Env,
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
        let contract = bytecode.contract.borrow(env)?;

        // there's a receive function and no calldata
        if trace.calldata.len() == 0 && contract.receive.is_some() {
            return Ok(false);
        }

        Ok(contract.fallback.is_none())
    }

    pub fn empty_calldata_and_no_receive(trace: &CallMessageTrace, env: Env) -> napi::Result<bool> {
        let bytecode = trace
            .bytecode
            .as_ref()
            .expect("The TS code type-checks this to always have bytecode");
        let contract = bytecode.contract.borrow(env)?;

        let version = Version::parse(&bytecode.compiler_version).unwrap();
        // this only makes sense when receive functions are available
        if version < FIRST_SOLC_VERSION_RECEIVE_FUNCTION {
            return Ok(false);
        }

        Ok(trace.calldata.is_empty() && contract.receive.is_none())
    }

    fn is_fallback_not_payable_error(
        trace: &CallMessageTrace,
        called_function: Option<&ContractFunction>,
        env: Env,
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
        let contract = bytecode.contract.borrow(env)?;

        match &contract.fallback {
            None => Ok(false),
            Some(fallback) => {
                let fallback = fallback.borrow(env)?;

                Ok(fallback.is_payable != Some(true))
            }
        }
    }

    #[napi(catch_unwind)]
    pub fn get_fallback_start_source_reference(
        trace: CallMessageTrace,
        env: Env,
    ) -> napi::Result<SourceReference> {
        Self::get_fallback_start_source_reference_inner(&trace, env)
    }

    fn get_fallback_start_source_reference_inner(
        trace: &CallMessageTrace,
        env: Env,
    ) -> napi::Result<SourceReference> {
        let bytecode = trace.bytecode.as_ref().expect("JS code asserts");
        let contract = bytecode.contract.borrow(env)?;

        let func = match &contract.fallback {
          Some(func) => func,
          None => panic!("This shouldn't happen: trying to get fallback source reference from a contract without fallback"),
        };

        let func = func.borrow(env)?;
        let location = func.location.borrow(env)?;
        let file = location.file.borrow(env)?;

        Ok(SourceReference {
            source_name: file.source_name.clone(),
            source_content: file.content.clone(),
            contract: Some(contract.name.clone()),
            function: Some(FALLBACK_FUNCTION_NAME.to_string()),
            line: location.get_starting_line_number(env).unwrap(),
            range: [location.offset, location.offset + location.length].to_vec(),
        })
    }

    fn is_constructor_not_payable_error(
        trace: &CreateMessageTrace,
        env: Env,
    ) -> napi::Result<bool> {
        // This error doesn't return data
        if !trace.return_data.is_empty() {
            return Ok(false);
        }

        let bytecode = trace.bytecode.as_ref().expect("JS code asserts");
        let contract = bytecode.contract.borrow(env)?;

        // This function is only matters with contracts that have constructors defined. The ones that
        // don't are abstract contracts, or their constructor doesn't take any argument.
        let constructor = match &contract.constructor {
            Some(constructor) => constructor,
            None => return Ok(false),
        };

        let constructor = constructor.borrow(env)?;

        let (neg, value, _) = trace.value.get_u64();
        if neg || value == 0 {
            return Ok(false);
        }

        Ok(constructor.is_payable != Some(true))
    }

    /// Returns a source reference pointing to the constructor if it exists, or to the contract otherwise.
    #[napi]
    pub fn get_constructor_start_source_reference(
        trace: CreateMessageTrace,
        env: Env,
    ) -> napi::Result<SourceReference> {
        Self::get_constructor_start_source_reference_inner(&trace, env)
    }

    fn get_constructor_start_source_reference_inner(
        trace: &CreateMessageTrace,
        env: Env,
    ) -> napi::Result<SourceReference> {
        let bytecode = trace.bytecode.as_ref().expect("JS code asserts");
        let contract = bytecode.contract.borrow(env)?;
        let contract_location = contract.location.borrow(env)?;

        let line = match &contract.constructor {
            Some(constructor) => {
                let constructor = constructor.borrow(env)?;
                let location = constructor.location.borrow(env)?;

                location.get_starting_line_number(env)?
            }
            None => contract_location.get_starting_line_number(env)?,
        };

        let file = contract_location.file.borrow(env)?;

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

    fn is_constructor_invalid_arguments_error(
        trace: &CreateMessageTrace,
        env: Env,
    ) -> napi::Result<bool> {
        if trace.return_data.len() > 0 {
            return Ok(false);
        }

        let bytecode = trace.bytecode.as_ref().expect("JS code asserts");
        let contract = bytecode.contract.borrow(env)?;

        // This function is only matters with contracts that have constructors defined. The ones that
        // don't are abstract contracts, or their constructor doesn't take any argument.
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

        let last_inst = bytecode.get_instruction_inner(last_step.pc)?;
        let last_inst = last_inst.borrow(env)?;

        if last_inst.opcode != Opcode::REVERT || last_inst.location.is_some() {
            return Ok(false);
        }

        let contract_location = contract.location.borrow(env)?;
        let constructor = constructor.borrow(env)?;
        let constructor_location = constructor.location.borrow(env)?;

        let mut has_read_deployment_code_size = false;
        for step in &trace.steps {
            let step = match step {
                Either4::A(step) => step,
                _ => return Ok(false),
            };

            let inst = bytecode.get_instruction_inner(step.pc)?;
            let inst = inst.borrow(env)?;
            let inst_location = inst.location.as_ref().map(|x| x.borrow(env)).transpose()?;

            if let Some(inst_location) = inst_location {
                if !contract_location.equals(&inst_location, env)
                    && !constructor_location.equals(&inst_location, env)
                {
                    return Ok(false);
                }
            }

            if inst.opcode == Opcode::CODESIZE {
                has_read_deployment_code_size = true;
            }
        }

        Ok(has_read_deployment_code_size)
    }

    #[napi]
    pub fn get_contract_start_without_function_source_reference(
        trace: Either<CallMessageTrace, CreateMessageTrace>,
        env: Env,
    ) -> napi::Result<SourceReference> {
        Self::get_contract_start_without_function_source_reference_inner(trace.as_ref(), env)
    }

    pub fn get_contract_start_without_function_source_reference_inner(
        trace: Either<&CallMessageTrace, &CreateMessageTrace>,
        env: Env,
    ) -> napi::Result<SourceReference> {
        let bytecode = match &trace {
            Either::A(create) => &create.bytecode,
            Either::B(call) => &call.bytecode,
        };

        let contract = &bytecode.as_ref().expect("JS code asserts").contract;

        let contract = contract.borrow(env)?;

        let location = contract.location.borrow(env)?;
        let file = location.file.borrow(env)?;

        Ok(SourceReference {
            source_name: file.source_name.clone(),
            source_content: file.content.clone(),
            contract: Some(contract.name.clone()),

            function: None,
            line: location.get_starting_line_number(env).unwrap(),
            range: [location.offset, location.offset + location.length].to_vec(),
        })
    }

    pub fn is_function_not_payable_error(
        trace: &CallMessageTrace,
        called_function: &ContractFunction,
        env: Env,
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
        let contract = bytecode.contract.borrow(env)?;

        // Libraries don't have a nonpayable check
        if contract.r#type == ContractType::LIBRARY {
            return Ok(false);
        }

        Ok(called_function.is_payable != Some(true))
    }

    #[napi]
    pub fn get_last_source_reference(
        trace: Either<CallMessageTrace, CreateMessageTrace>,
        env: Env,
    ) -> napi::Result<Either<SourceReference, Undefined>> {
        let trace = match &trace {
            Either::A(call) => Either::A(call),
            Either::B(create) => Either::B(create),
        };

        Self::get_last_source_reference_inner(trace, env)
    }

    pub fn get_last_source_reference_inner(
        trace: Either<&CallMessageTrace, &CreateMessageTrace>,
        env: Env,
    ) -> napi::Result<Either<SourceReference, Undefined>> {
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

            let inst = bytecode.get_instruction_inner(step.pc)?;

            let location = &inst.borrow(env)?.location;
            let Some(location) = location else {
                continue;
            };
            let location = location.borrow(env)?;

            let source_reference =
                source_location_to_source_reference(bytecode, Some(&*location), env)?;

            if let Some(source_reference) = source_reference {
                return Ok(Either::A(source_reference));
            }
        }

        Ok(Either::B(()))
    }

    #[napi]
    pub fn has_failed_inside_the_fallback_function(
        trace: CallMessageTrace,
        env: Env,
    ) -> napi::Result<bool> {
        Self::has_failed_inside_the_fallback_function_inner(&trace, env)
    }

    pub fn has_failed_inside_the_fallback_function_inner(
        trace: &CallMessageTrace,
        env: Env,
    ) -> napi::Result<bool> {
        let contract = &trace.bytecode.as_ref().expect("JS code asserts").contract;
        let contract = contract.borrow(env)?;

        match &contract.fallback {
            Some(fallback) => {
                let fallback = fallback.borrow(env)?;
                Self::has_failed_inside_function(trace, &fallback, env)
            }
            None => Ok(false),
        }
    }

    #[napi]
    pub fn has_failed_inside_the_receive_function(
        trace: CallMessageTrace,
        env: Env,
    ) -> napi::Result<bool> {
        Self::has_failed_inside_the_receive_function_inner(&trace, env)
    }

    pub fn has_failed_inside_the_receive_function_inner(
        trace: &CallMessageTrace,
        env: Env,
    ) -> napi::Result<bool> {
        let contract = &trace.bytecode.as_ref().expect("JS code asserts").contract;
        let contract = contract.borrow(env)?;

        match &contract.receive {
            Some(receive) => {
                let receive = receive.borrow(env)?;
                Self::has_failed_inside_function(trace, &receive, env)
            }
            None => Ok(false),
        }
    }

    pub fn has_failed_inside_function(
        trace: &CallMessageTrace,
        func: &ContractFunction,
        env: Env,
    ) -> napi::Result<bool> {
        let last_step = trace.steps.last().unwrap();
        let last_step = match last_step {
            Either4::A(step) => step,
            _ => panic!("JS code asserted this is always an EvmStep"),
        };

        let last_instruction = trace
            .bytecode
            .as_ref()
            .expect("The TS code type-checks this to always have bytecode")
            .get_instruction_inner(last_step.pc)?;

        let last_instruction = last_instruction.borrow(env)?;
        let last_instruction_location = last_instruction
            .location
            .as_ref()
            .map(|i| i.borrow(env))
            .transpose()?;

        Ok(match last_instruction_location {
            Some(last_instruction_location) => {
                last_instruction.opcode == Opcode::REVERT
                    && func
                        .location
                        .borrow(env)?
                        .contains(&last_instruction_location, env)
            }
            _ => false,
        })
    }

    #[napi(catch_unwind)]
    pub fn instruction_within_function_to_revert_stack_trace_entry(
        trace: Either<CallMessageTrace, CreateMessageTrace>,
        inst: &Instruction,
        env: Env,
    ) -> napi::Result<RevertErrorStackTraceEntry> {
        Self::instruction_within_function_to_revert_stack_trace_entry_inner(
            trace.as_ref(),
            inst,
            env,
        )
    }

    pub fn instruction_within_function_to_revert_stack_trace_entry_inner(
        trace: Either<&CallMessageTrace, &CreateMessageTrace>,
        inst: &Instruction,
        env: Env,
    ) -> napi::Result<RevertErrorStackTraceEntry> {
        let bytecode = match &trace {
            Either::A(create) => &create.bytecode,
            Either::B(call) => &call.bytecode,
        }
        .as_ref()
        .expect("JS code asserts");

        let inst_location = inst.location.as_ref().map(|i| i.borrow(env)).transpose()?;
        let inst_location = inst_location.as_deref();

        let source_reference = source_location_to_source_reference(bytecode, inst_location, env)?
            .expect("Expected source reference to be defined");

        let return_data = match &trace {
            Either::A(create) => &create.return_data,
            Either::B(call) => &call.return_data,
        };

        Ok(RevertErrorStackTraceEntry {
            type_: StackTraceEntryTypeConst,
            source_reference,
            is_invalid_opcode_error: inst.opcode == Opcode::INVALID,
            message: ReturnData::new(return_data.clone()).into_instance(env)?,
        })
    }

    #[napi]
    pub fn instruction_within_function_to_unmapped_solc_0_6_3_revert_error_stack_trace_entry(
        trace: Either<CallMessageTrace, CreateMessageTrace>,
        inst: &Instruction,
        env: Env,
    ) -> napi::Result<UnmappedSolc063RevertErrorStackTraceEntry> {
        Self::instruction_within_function_to_unmapped_solc_0_6_3_revert_error_stack_trace_entry_inner(
        trace.as_ref(),
        inst,
        env
      )
    }

    pub fn instruction_within_function_to_unmapped_solc_0_6_3_revert_error_stack_trace_entry_inner(
        trace: Either<&CallMessageTrace, &CreateMessageTrace>,
        inst: &Instruction,
        env: Env,
    ) -> napi::Result<UnmappedSolc063RevertErrorStackTraceEntry> {
        let bytecode = match &trace {
            Either::A(create) => &create.bytecode,
            Either::B(call) => &call.bytecode,
        }
        .as_ref()
        .expect("JS code asserts");

        let inst_location = inst.location.as_ref().map(|i| i.borrow(env)).transpose()?;
        let inst_location = inst_location.as_deref();

        let source_reference = source_location_to_source_reference(bytecode, inst_location, env)?;

        Ok(UnmappedSolc063RevertErrorStackTraceEntry {
            type_: StackTraceEntryTypeConst,
            source_reference,
        })
    }

    #[napi]
    pub fn instruction_within_function_to_panic_stack_trace_entry(
        trace: Either<CallMessageTrace, CreateMessageTrace>,
        inst: &Instruction,
        error_code: BigInt,
        env: Env,
    ) -> napi::Result<PanicErrorStackTraceEntry> {
        Self::instruction_within_function_to_panic_stack_trace_entry_inner(
            trace.as_ref(),
            inst,
            error_code,
            env,
        )
    }

    pub fn instruction_within_function_to_panic_stack_trace_entry_inner(
        trace: Either<&CallMessageTrace, &CreateMessageTrace>,
        inst: &Instruction,
        error_code: BigInt,
        env: Env,
    ) -> napi::Result<PanicErrorStackTraceEntry> {
        let last_source_reference = Self::get_last_source_reference_inner(trace, env)?;

        let bytecode = match &trace {
            Either::A(create) => &create.bytecode,
            Either::B(call) => &call.bytecode,
        }
        .as_ref()
        .expect("JS code asserts");

        let inst_location = inst.location.as_ref().map(|i| i.borrow(env)).transpose()?;
        let inst_location = inst_location.as_deref();

        let source_reference = source_location_to_source_reference(bytecode, inst_location, env)?;

        let source_reference = source_reference.or(last_source_reference.into_option());

        Ok(PanicErrorStackTraceEntry {
            type_: StackTraceEntryTypeConst,
            source_reference,
            error_code,
        })
    }

    #[napi(catch_unwind)]
    pub fn instruction_within_function_to_custom_error_stack_trace_entry(
        trace: Either<CallMessageTrace, CreateMessageTrace>,
        inst: &Instruction,
        message: String,
        env: Env,
    ) -> napi::Result<CustomErrorStackTraceEntry> {
        Self::instruction_within_function_to_custom_error_stack_trace_entry_inner(
            trace.as_ref(),
            inst,
            message,
            env,
        )
    }

    pub fn instruction_within_function_to_custom_error_stack_trace_entry_inner(
        trace: Either<&CallMessageTrace, &CreateMessageTrace>,
        inst: &Instruction,
        message: String,
        env: Env,
    ) -> napi::Result<CustomErrorStackTraceEntry> {
        let last_source_reference = Self::get_last_source_reference_inner(trace, env)?;
        let last_source_reference = match last_source_reference {
            Either::A(source_reference) => source_reference,
            Either::B(_) => panic!("Expected source reference to be defined"),
        };

        let bytecode = match &trace {
            Either::A(create) => &create.bytecode,
            Either::B(call) => &call.bytecode,
        }
        .as_ref()
        .expect("JS code asserts");

        let inst_location = inst.location.as_ref().map(|i| i.borrow(env)).transpose()?;
        let inst_location = inst_location.as_deref();

        let source_reference = source_location_to_source_reference(bytecode, inst_location, env)?;

        let source_reference = source_reference.unwrap_or(last_source_reference);

        Ok(CustomErrorStackTraceEntry {
            type_: StackTraceEntryTypeConst,
            source_reference,
            message,
        })
    }

    #[napi]
    pub fn solidity_0_6_3_maybe_unmapped_revert(
        trace: Either<CallMessageTrace, CreateMessageTrace>,
        env: Env,
    ) -> napi::Result<bool> {
        Self::solidity_0_6_3_maybe_unmapped_revert_inner(trace.as_ref(), env)
    }

    pub fn solidity_0_6_3_maybe_unmapped_revert_inner(
        trace: Either<&CallMessageTrace, &CreateMessageTrace>,
        env: Env,
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

        let last_instruction = bytecode.get_instruction_inner(last_step.pc)?;

        let Ok(version) = Version::parse(&bytecode.compiler_version) else {
            return Ok(false);
        };
        let req = VersionReq::parse(&format!("^{FIRST_SOLC_VERSION_WITH_UNMAPPED_REVERTS}"))
            .expect("valid semver");

        Ok(req.matches(&version) && last_instruction.borrow(env)?.opcode == Opcode::REVERT)
    }

    // Solidity 0.6.3 unmapped reverts special handling
    // For more info: https://github.com/ethereum/solidity/issues/9006
    #[napi]
    pub fn solidity_0_6_3_get_frame_for_unmapped_revert_before_function(
        trace: CallMessageTrace,
        env: Env,
    ) -> napi::Result<Either<UnmappedSolc063RevertErrorStackTraceEntry, Undefined>> {
        Self::solidity_0_6_3_get_frame_for_unmapped_revert_before_function_inner(&trace, env)
    }

    // Solidity 0.6.3 unmapped reverts special handling
    // For more info: https://github.com/ethereum/solidity/issues/9006
    pub fn solidity_0_6_3_get_frame_for_unmapped_revert_before_function_inner(
        trace: &CallMessageTrace,
        env: Env,
    ) -> napi::Result<Either<UnmappedSolc063RevertErrorStackTraceEntry, Undefined>> {
        let bytecode = trace.bytecode.as_ref().expect("JS code asserts");
        let contract = bytecode.contract.borrow(env)?;

        let revert_frame =
            Self::solidity_0_6_3_get_frame_for_unmapped_revert_within_function_inner(
                Either::A(trace),
                env,
            )?;

        let revert_frame = match revert_frame.into_option() {
            None
            | Some(UnmappedSolc063RevertErrorStackTraceEntry {
                source_reference: None,
                ..
            }) => {
                if contract.receive.is_none() || trace.calldata.len() > 0 {
                    // Failed within the fallback
                    if let Some(fallback) = &contract.fallback {
                        let fallback = fallback.borrow(env)?;
                        let location = fallback.location.borrow(env)?;
                        let file = location.file.borrow(env)?;

                        let revert_frame = UnmappedSolc063RevertErrorStackTraceEntry {
                            type_: StackTraceEntryTypeConst,
                            source_reference: Some(SourceReference {
                                contract: Some(contract.name.clone()),
                                function: Some(FALLBACK_FUNCTION_NAME.to_string()),
                                source_name: file.source_name.clone(),
                                source_content: file.content.clone(),
                                line: location.get_starting_line_number(env).unwrap(),
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

                    let receive = receive.borrow(env)?;
                    let location = receive.location.borrow(env)?;
                    let file = location.file.borrow(env)?;

                    let revert_frame = UnmappedSolc063RevertErrorStackTraceEntry {
                        type_: StackTraceEntryTypeConst,
                        source_reference: Some(SourceReference {
                            contract: Some(contract.name.clone()),
                            function: Some(RECEIVE_FUNCTION_NAME.to_string()),
                            source_name: file.source_name.clone(),
                            source_content: file.content.clone(),
                            line: location.get_starting_line_number(env).unwrap(),
                            range: [location.offset, location.offset + location.length].to_vec(),
                        }),
                    };

                    Some(Self::solidity_0_6_3_correct_line_number(revert_frame))
                }
            }
            Some(revert_frame) => Some(revert_frame),
        };

        Ok(revert_frame.into_either())
    }

    #[napi]
    pub fn solidity_0_6_3_get_frame_for_unmapped_revert_within_function(
        trace: Either<CallMessageTrace, CreateMessageTrace>,
        env: Env,
    ) -> napi::Result<Either<UnmappedSolc063RevertErrorStackTraceEntry, Undefined>> {
        Self::solidity_0_6_3_get_frame_for_unmapped_revert_within_function_inner(
            trace.as_ref(),
            env,
        )
    }

    pub fn solidity_0_6_3_get_frame_for_unmapped_revert_within_function_inner(
        trace: Either<&CallMessageTrace, &CreateMessageTrace>,
        env: Env,
    ) -> napi::Result<Either<UnmappedSolc063RevertErrorStackTraceEntry, Undefined>> {
        let (bytecode, steps) = match &trace {
            Either::A(create) => (&create.bytecode, &create.steps),
            Either::B(call) => (&call.bytecode, &call.steps),
        };

        let bytecode = bytecode
            .as_ref()
            .expect("JS code only accepted variants that had bytecode defined");

        let contract = bytecode.contract.borrow(env)?;

        // If we are within a function there's a last valid location. It may
        // be the entire contract.
        let prev_inst = Self::get_last_instruction_with_valid_location(trace, env)?;
        let prev_inst = prev_inst.into_option().map(|x| x.borrow(env)).transpose()?;
        let last_step = match steps.last() {
            Some(Either4::A(step)) => step,
            _ => panic!("JS code asserts this is always an EvmStep"),
        };
        let next_inst_pc = last_step.pc + 1;
        let has_next_inst = bytecode.has_instruction(next_inst_pc);

        if has_next_inst {
            let next_inst = bytecode.get_instruction_inner(next_inst_pc)?;
            let next_inst = next_inst.borrow(env)?;

            let prev_loc = prev_inst
                .as_ref()
                .and_then(|x| x.location.as_ref())
                .map(|l| l.borrow(env))
                .transpose()?;
            let next_loc = next_inst
                .location
                .as_ref()
                .map(|l| l.borrow(env))
                .transpose()?;

            let prev_func = prev_loc
                .as_ref()
                .map(|l| l.get_containing_function_inner(env))
                .transpose()?
                .and_then(IntoOption::into_option);

            let next_func = next_loc
                .as_ref()
                .map(|l| l.get_containing_function_inner(env))
                .transpose()?
                .and_then(IntoOption::into_option);

            // This is probably a require. This means that we have the exact
            // line, but the stack trace may be degraded (e.g. missing our
            // synthetic call frames when failing in a modifier) so we still
            // add this frame as UNMAPPED_SOLC_0_6_3_REVERT_ERROR
            match (&prev_func, &next_loc, &prev_loc) {
                (Some(_), Some(next_loc), Some(prev_loc)) if prev_loc.equals(next_loc, env) => {
                    return Ok(Either::A(Self::instruction_within_function_to_unmapped_solc_0_6_3_revert_error_stack_trace_entry_inner(
                trace,
                &next_inst,
                env,
              )?));
                }
                _ => {}
            }

            let mut revert_frame: Either<UnmappedSolc063RevertErrorStackTraceEntry, Undefined> =
                Either::B(());

            if prev_func.is_some() && prev_inst.is_some() {
                revert_frame = Either::A(Self::instruction_within_function_to_unmapped_solc_0_6_3_revert_error_stack_trace_entry_inner(
                trace,
                prev_inst.as_ref().unwrap(),
                env,
              )?);
            } else if next_func.is_some() {
                revert_frame = Either::A(Self::instruction_within_function_to_unmapped_solc_0_6_3_revert_error_stack_trace_entry_inner(
                trace,
                &next_inst,
                env,
              )?);
            }

            return Ok(revert_frame
                .into_option()
                .map(Self::solidity_0_6_3_correct_line_number)
                .into_either());
        }

        if matches!(trace, Either::B(CreateMessageTrace { .. })) && prev_inst.is_some() {
            // Solidity is smart enough to stop emitting extra instructions after
            // an unconditional revert happens in a constructor. If this is the case
            // we just return a special error.

            let mut constructor_revert_frame = Self::instruction_within_function_to_unmapped_solc_0_6_3_revert_error_stack_trace_entry_inner(
                trace,
                prev_inst.as_ref().unwrap(),
                env,
            )?;

            // When the latest instruction is not within a function we need
            // some default sourceReference to show to the user
            if constructor_revert_frame.source_reference.is_none() {
                let location = contract.location.borrow(env)?;
                let file = location.file.borrow(env)?;

                let mut default_source_reference = SourceReference {
                    function: Some(CONSTRUCTOR_FUNCTION_NAME.to_string()),
                    contract: Some(contract.name.clone()),
                    source_name: file.source_name.clone(),
                    source_content: file.content.clone(),
                    line: location.get_starting_line_number(env)?,
                    range: [location.offset, location.offset + location.length].to_vec(),
                };

                if let Some(constructor) = &contract.constructor {
                    default_source_reference.line = constructor
                        .borrow(env)?
                        .location
                        .borrow(env)?
                        .get_starting_line_number(env)?;
                }

                constructor_revert_frame.source_reference = Some(default_source_reference);
            } else {
                constructor_revert_frame =
                    Self::solidity_0_6_3_correct_line_number(constructor_revert_frame);
            }

            return Ok(Either::A(constructor_revert_frame));
        }

        if prev_inst.is_some() {
            // We may as well just be in a function or modifier and just happen
            // to be at the last instruction of the runtime bytecode.
            // In this case we just return whatever the last mapped intruction
            // points to.
            let mut latest_instruction_revert_frame = Self::instruction_within_function_to_unmapped_solc_0_6_3_revert_error_stack_trace_entry_inner(
                trace,
                prev_inst.as_ref().unwrap(),
                env,
            )?;

            if latest_instruction_revert_frame.source_reference.is_some() {
                latest_instruction_revert_frame =
                    Self::solidity_0_6_3_correct_line_number(latest_instruction_revert_frame);
            }
            return Ok(Either::A(latest_instruction_revert_frame));
        }

        Ok(Either::B(()))
    }

    #[napi]
    pub fn solidity_0_6_3_correct_line_number(
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

    #[napi]
    pub fn get_other_error_before_called_function_stack_trace_entry(
        trace: CallMessageTrace,
        env: Env,
    ) -> napi::Result<OtherExecutionErrorStackTraceEntry> {
        Self::get_other_error_before_called_function_stack_trace_entry_inner(&trace, env)
    }

    pub fn get_other_error_before_called_function_stack_trace_entry_inner(
        trace: &CallMessageTrace,
        env: Env,
    ) -> napi::Result<OtherExecutionErrorStackTraceEntry> {
        let source_reference = Self::get_contract_start_without_function_source_reference_inner(
            Either::A(trace),
            env,
        )?;

        Ok(OtherExecutionErrorStackTraceEntry {
            type_: StackTraceEntryTypeConst,
            source_reference: Some(source_reference),
        })
    }

    fn is_called_non_contract_account_error(
        trace: Either<&CallMessageTrace, &CreateMessageTrace>,
        env: Env,
    ) -> napi::Result<bool> {
        // We could change this to checking that the last valid location maps to a call, but
        // it's way more complex as we need to get the ast node from that location.

        let (bytecode, steps) = match &trace {
            Either::A(create) => (&create.bytecode, &create.steps),
            Either::B(call) => (&call.bytecode, &call.steps),
        };

        let bytecode = bytecode
            .as_ref()
            .expect("JS code only accepted variants that had bytecode defined");

        let last_index = Self::get_last_instruction_with_valid_location_step_index(trace, env)?;

        let last_index = match last_index.into_option() {
            None | Some(0) => return Ok(false),
            Some(last_index) => last_index as usize,
        };

        let last_step = match &steps[last_index] {
            Either4::A(step) => step,
            _ => panic!("We know this is an EVM step"),
        };

        let last_inst = bytecode.get_instruction_inner(last_step.pc)?;
        let last_inst = last_inst.borrow(env)?;

        if last_inst.opcode != Opcode::ISZERO {
            return Ok(false);
        }

        let prev_step = match &steps[last_index - 1] {
            Either4::A(step) => step,
            _ => panic!("We know this is an EVM step"),
        };

        let prev_inst = bytecode.get_instruction_inner(prev_step.pc)?;
        let prev_inst = prev_inst.borrow(env)?;

        Ok(prev_inst.opcode == Opcode::EXTCODESIZE)
    }

    fn get_last_instruction_with_valid_location_step_index(
        trace: Either<&CallMessageTrace, &CreateMessageTrace>,
        env: Env,
    ) -> napi::Result<Either<u32, Undefined>> {
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
                _ => return Ok(Either::B(())),
            };

            let inst = bytecode.get_instruction_inner(step.pc)?;

            let inst = inst.borrow(env)?;
            if inst.location.is_some() {
                return Ok(Either::A(i as u32));
            }
        }

        Ok(Either::B(()))
    }

    pub fn get_last_instruction_with_valid_location<'a>(
        trace: Either<&'a CallMessageTrace, &'a CreateMessageTrace>,
        env: Env,
    ) -> napi::Result<Either<&'a ClassInstanceRef<Instruction>, Undefined>> {
        let last_location_index =
            Self::get_last_instruction_with_valid_location_step_index(trace, env)?;

        let last_location_index = match last_location_index {
            Either::A(last_location_index) => last_location_index,
            Either::B(_) => return Ok(Either::B(())),
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
                let inst = bytecode.get_instruction_inner(step.pc)?;

                Ok(Either::A(inst))
            }
            _ => Ok(Either::B(())),
        }
    }

    #[napi]
    pub fn is_panic_return_data(return_data: Uint8Array) -> bool {
        ReturnData::new(return_data).is_panic_return_data()
    }
}

fn source_location_to_source_reference(
    bytecode: &Bytecode,
    location: Option<&SourceLocation>,
    env: Env,
) -> napi::Result<Option<SourceReference>> {
    let Some(location) = location else {
        return Ok(None);
    };

    let func = location.get_containing_function_inner(env)?;

    let Either::A(func) = func else {
        return Ok(None);
    };

    let func = func.borrow(env)?;

    let func_name = match func.r#type {
        ContractFunctionType::CONSTRUCTOR => CONSTRUCTOR_FUNCTION_NAME.to_string(),
        ContractFunctionType::FALLBACK => FALLBACK_FUNCTION_NAME.to_string(),
        ContractFunctionType::RECEIVE => RECEIVE_FUNCTION_NAME.to_string(),
        _ => func.name.clone(),
    };

    let func_location = func.location.borrow(env)?;
    let func_location_file = func_location.file.borrow(env)?;

    Ok(Some(SourceReference {
        function: Some(func_name.clone()),
        contract: if func.r#type == ContractFunctionType::FREE_FUNCTION {
            None
        } else {
            Some(bytecode.contract.borrow(env)?.name.clone())
        },
        source_name: func_location_file.source_name.clone(),
        source_content: func_location_file.content.clone(),
        line: location.get_starting_line_number(env)?,
        range: [location.offset, location.offset + location.length].to_vec(),
    }))
}

#[napi(catch_unwind)]
pub fn instruction_to_callstack_stack_trace_entry(
    bytecode: &Bytecode,
    inst: &Instruction,
    env: Env,
) -> napi::Result<Either<CallstackEntryStackTraceEntry, InternalFunctionCallStackEntry>> {
    let contract = bytecode.contract.borrow(env)?;

    // This means that a jump is made from within an internal solc function.
    // These are normally made from yul code, so they don't map to any Solidity
    // function
    let inst_location = match &inst.location {
        None => {
            let location = contract.location.borrow(env)?;
            let file = location.file.borrow(env)?;

            return Ok(Either::B(InternalFunctionCallStackEntry {
                type_: StackTraceEntryTypeConst,
                pc: inst.pc,
                source_reference: SourceReference {
                    source_name: file.source_name.clone(),
                    source_content: file.content.clone(),
                    contract: Some(contract.name.clone()),
                    function: None,
                    line: location.get_starting_line_number(env)?,
                    range: [location.offset, location.offset + location.length].to_vec(),
                },
            }));
        }
        Some(inst_location) => inst_location.borrow(env)?,
    };

    let func = inst_location.get_containing_function_inner(env)?;

    if let Some(func) = func.into_option() {
        let func = func.borrow(env)?;

        let source_reference =
            source_location_to_source_reference(bytecode, Some(&*inst_location), env)?
                .expect("Expected source reference to be defined");

        return Ok(Either::A(CallstackEntryStackTraceEntry {
            type_: StackTraceEntryTypeConst,
            source_reference,
            function_type: func.r#type,
        }));
    };

    let file = inst_location.file.borrow(env)?;

    Ok(Either::A(CallstackEntryStackTraceEntry {
        type_: StackTraceEntryTypeConst,
        source_reference: SourceReference {
            function: None,
            contract: Some(contract.name.clone()),
            source_name: file.source_name.clone(),
            source_content: file.content.clone(),
            line: inst_location.get_starting_line_number(env)?,
            range: [
                inst_location.offset,
                inst_location.offset + inst_location.length,
            ]
            .to_vec(),
        },
        function_type: ContractFunctionType::FUNCTION,
    }))
}

pub fn format_dyn_sol_value(val: &DynSolValue) -> String {
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

        DynSolValue::Address(address) => format!("\"0x{address}\""),
        DynSolValue::Bytes(bytes) => format!("\"0x{}\"", hex::encode(bytes)),
        DynSolValue::FixedBytes(word, size) => {
            format!("\"0x{}\"", hex::encode(&word.0.as_slice()[..*size]))
        }
        DynSolValue::Bool(b) => b.to_string(),
        DynSolValue::Function(_) => "<function>".to_string(),
        DynSolValue::Int(int, _bits) => int.to_string(),
        DynSolValue::Uint(uint, _bits) => uint.to_string(),
    }
}
