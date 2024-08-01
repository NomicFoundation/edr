use std::collections::HashSet;

use napi::bindgen_prelude::Either24;
use napi_derive::napi;

use crate::trace::solidity_stack_trace::ReturndataSizeErrorStackTraceEntry;

use super::solidity_stack_trace::{
    CallstackEntryStackTraceEntry, SolidityStackTrace, SolidityStackTraceEntryExt,
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
