use edr_solidity::solidity_stack_trace::StackTraceCreationResult;
use napi::{bindgen_prelude::Either3, Either};
use napi_derive::napi;

use crate::{
    gc::gc_tracked,
    solidity_tests::test_results::{CallTrace, HeuristicFailed, StackTrace, UnexpectedError},
    trace::solidity_stack_trace::{
        solidity_stack_trace_error_to_napi, solidity_stack_trace_heuristic_failed_to_napi,
        solidity_stack_trace_success_to_napi,
    },
};

/// Bytes reported for a response whose call trace arenas were recorded without
/// stack snapshots.
///
/// A `CallTraceStep` is 192 bytes, so this is a trace of roughly 5,400 steps.
/// A complex contract call runs many more, which is the intended direction:
/// over-reporting costs a collection on every request, while under-reporting
/// only forgoes some of the benefit.
const CALL_TRACE_EXTERNAL_MEM_SIZE: i64 = 1024 * 1024;

/// Bytes reported for a response whose call trace steps also carry stack
/// snapshots, as `verbose_raw_tracing` records them.
///
/// A snapshot is a `Box<[U256]>`, so a step costs roughly 448 bytes rather than
/// 192. PR #1301 measured 4.9x more retained memory in this configuration, so
/// 4x stays on the low side of the only figures available.
const VERBOSE_CALL_TRACE_EXTERNAL_MEM_SIZE: i64 = 4 * 1024 * 1024;

/// Bytes reported for a stack trace, which is a handful of frames of strings.
const STACK_TRACE_EXTERNAL_MEM_SIZE: i64 = 4 * 1024;

/// Bytes reported when the envelope was too large to marshal as a string.
///
/// That only happens above the 250 MB limit in
/// [`edr_napi_core::spec::marshal_response_data`], and a `serde_json::Value`
/// tree costs several times its serialized length, so this is a floor.
const VALUE_DATA_EXTERNAL_MEM_SIZE: i64 = 256 * 1024 * 1024;

/// Bytes a response reports for the call trace arenas it carries.
///
/// Recording stack snapshots takes a step from roughly 192 bytes to 448, so a
/// verbosely traced response stands for several times as much.
pub(crate) const fn call_trace_external_mem_size(verbose_tracing: bool) -> i64 {
    if verbose_tracing {
        VERBOSE_CALL_TRACE_EXTERNAL_MEM_SIZE
    } else {
        CALL_TRACE_EXTERNAL_MEM_SIZE
    }
}

#[napi(custom_finalize)]
pub struct Response {
    inner: edr_napi_core::spec::Response,
    /// Reported to V8 on the way in and released on finalize, so it is stored
    /// rather than recomputed.
    external_memory: i64,
}

impl Response {
    /// Constructs a new instance.
    ///
    /// `call_trace_external_mem_size` is what the provider reports for a
    /// response carrying call trace arenas, which depends on whether it records
    /// stack snapshots.
    pub(crate) fn new(
        inner: edr_napi_core::spec::Response,
        call_trace_external_mem_size: i64,
    ) -> Self {
        let mut external_memory = match &inner.data {
            Either::A(envelope) => i64::try_from(envelope.len()).unwrap_or(i64::MAX),
            Either::B(_value) => VALUE_DATA_EXTERNAL_MEM_SIZE,
        };

        if inner.stack_trace_result.is_some() {
            external_memory += STACK_TRACE_EXTERNAL_MEM_SIZE;
        }

        if !inner.call_trace_arenas.is_empty() {
            external_memory += call_trace_external_mem_size;
        }

        Self {
            inner,
            external_memory,
        }
    }
}

gc_tracked! {
    /// A [`Response`] on its way to JavaScript, reporting what it holds to V8.
    pub(crate) type GcResponse = GcTracked<Response>;

    /// Computed once by [`Response::new`].
    fn external_memory(&self) -> i64 {
        self.external_memory
    }

    fn drop(self) {
        // Nothing beyond dropping the response itself.
    }
}

#[napi]
impl Response {
    #[doc = "Returns the response data as a JSON string or a JSON object."]
    #[napi(catch_unwind, getter)]
    pub fn data(&self) -> Either<String, serde_json::Value> {
        self.inner.data.clone()
    }

    #[doc = "Compute the error stack trace. Return the stack trace if it can be decoded, otherwise returns none. Throws if there was an error computing the stack trace."]
    #[napi(catch_unwind)]
    pub fn stack_trace(&self) -> Option<Either3<StackTrace, UnexpectedError, HeuristicFailed>> {
        self.inner
            .stack_trace_result
            .as_ref()
            .map(|stack_trace_result| match stack_trace_result {
                StackTraceCreationResult::Success(stack_trace) => {
                    Either3::A(solidity_stack_trace_success_to_napi(stack_trace))
                }
                StackTraceCreationResult::Error(error) => {
                    Either3::B(solidity_stack_trace_error_to_napi(error))
                }
                StackTraceCreationResult::HeuristicFailed => {
                    Either3::C(solidity_stack_trace_heuristic_failed_to_napi())
                }
            })
    }

    /// Constructs the execution traces for the request. Returns an empty array
    /// if traces are not enabled for this provider according to
    /// [`crate::solidity_tests::config::SolidityTestRunnerConfigArgs::include_traces`]. Otherwise, returns
    /// an array of the root calls of the trace, which always includes the
    /// request's call itself.
    #[napi(catch_unwind)]
    pub fn call_traces(&self) -> Vec<CallTrace> {
        self.inner
            .call_trace_arenas
            .iter()
            .map(|call_trace_arena| CallTrace::from_arena_node(call_trace_arena, 0))
            .collect()
    }

    // TODO(#1288): Add backwards compatibility layer for Hardhat 2
    // #[doc = "Returns the raw traces of executed contracts. This maybe contain
    // zero or more traces."] #[napi(catch_unwind, getter)]
    // pub fn traces(&self) -> Vec<RawTrace> {
    //     self.inner
    //         .call_trace_arenas
    //         .iter()
    //         .map(|trace| RawTrace::from(trace.clone()))
    //         .collect()
    // }
}

#[cfg(test)]
mod tests {
    use edr_solidity_tests::traces::CallTraceArena;

    use super::*;

    fn response(data: edr_napi_core::spec::ResponseData) -> edr_napi_core::spec::Response {
        edr_napi_core::spec::Response {
            data,
            stack_trace_result: None,
            call_trace_arenas: Vec::new(),
        }
    }

    #[test]
    fn external_memory_of_an_untraced_response_is_its_envelope() {
        let envelope = "a".repeat(2_048);
        let response = Response::new(
            response(Either::A(envelope.clone())),
            call_trace_external_mem_size(false),
        );

        assert_eq!(response.external_memory, envelope.len() as i64);
    }

    #[test]
    fn external_memory_counts_a_stack_trace() {
        let mut inner = response(Either::A("a".repeat(2_048)));
        inner.stack_trace_result = Some(StackTraceCreationResult::HeuristicFailed);

        let response = Response::new(inner, call_trace_external_mem_size(false));

        assert_eq!(
            response.external_memory,
            2_048 + STACK_TRACE_EXTERNAL_MEM_SIZE
        );
    }

    /// Whether traces were requested is not the question, so the arenas being
    /// present is what the figure keys on.
    #[test]
    fn external_memory_counts_recorded_arenas() {
        let mut inner = response(Either::A("a".repeat(2_048)));
        inner.call_trace_arenas = vec![CallTraceArena::default()];

        let response = Response::new(inner, call_trace_external_mem_size(false));

        assert_eq!(
            response.external_memory,
            2_048 + call_trace_external_mem_size(false)
        );
    }

    #[test]
    fn external_memory_uses_the_verbose_figure_when_given_it() {
        let mut inner = response(Either::A(String::new()));
        inner.call_trace_arenas = vec![CallTraceArena::default()];

        let response = Response::new(inner, call_trace_external_mem_size(true));

        assert_eq!(response.external_memory, call_trace_external_mem_size(true));
    }

    /// Pins the ratio the figures were derived from: PR #1301 measured 4.9x
    /// more retained memory with stack snapshots, so 4x stays on the low side.
    #[test]
    fn recording_stack_snapshots_reports_four_times_as_much() {
        assert_eq!(
            call_trace_external_mem_size(true),
            4 * call_trace_external_mem_size(false)
        );
    }

    /// The `Value` arm is only reachable above the marshalling limit, so its
    /// figure is a floor rather than an estimate of the envelope.
    #[test]
    fn external_memory_of_an_oversized_response_is_the_value_floor() {
        let response = Response::new(
            response(Either::B(serde_json::Value::Null)),
            call_trace_external_mem_size(false),
        );

        assert_eq!(response.external_memory, VALUE_DATA_EXTERNAL_MEM_SIZE);
    }
}
