use std::collections::HashSet;

use napi::{
    bindgen_prelude::{Either24, Undefined},
    Either, Env,
};
use napi_derive::napi;

use crate::trace::{
    model::ContractFunctionType,
    solidity_stack_trace::{
        ReturndataSizeErrorStackTraceEntry, CONSTRUCTOR_FUNCTION_NAME, FALLBACK_FUNCTION_NAME,
        RECEIVE_FUNCTION_NAME,
    },
};

use super::{
    model::{Bytecode, SourceLocation},
    solidity_stack_trace::{
        CallstackEntryStackTraceEntry, SolidityStackTrace, SolidityStackTraceEntryExt,
        SourceReference,
    },
};

#[napi]
pub struct ErrorInferrer;

#[napi]
impl ErrorInferrer {
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
}

#[napi]
pub fn source_location_to_source_reference(
    bytecode: &Bytecode,
    location: Either<&SourceLocation, Undefined>,
    env: Env,
) -> napi::Result<Either<SourceReference, Undefined>> {
    let Either::A(location) = location else {
        return Ok(Either::B(()));
    };

    let func = location.get_containing_function_inner(env)?;

    let Either::A(func) = func else {
        return Ok(Either::B(()));
    };

    let mut func = func.borrow_mut(env)?;

    if func.r#type == ContractFunctionType::CONSTRUCTOR {
        func.name = CONSTRUCTOR_FUNCTION_NAME.to_string();
    } else if func.r#type == ContractFunctionType::FALLBACK {
        func.name = FALLBACK_FUNCTION_NAME.to_string();
    } else if func.r#type == ContractFunctionType::RECEIVE {
        func.name = RECEIVE_FUNCTION_NAME.to_string();
    }

    let func_location = func.location.borrow(env)?;
    let func_location_file = func_location.file.borrow(env)?;

    Ok(Either::A(SourceReference {
        function: Some(func.name.clone()),
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
