use edr_solidity::solidity_stack_trace::StackTraceCreationResult;
use napi::{
    bindgen_prelude::{Either3, ObjectFinalize, ToNapiValue},
    Either,
};
use napi_derive::napi;

use crate::{
    async_deallocator::AsyncDeallocatorSender,
    solidity_tests::test_results::{CallTrace, HeuristicFailed, StackTrace, UnexpectedError},
    trace::solidity_stack_trace::{
        solidity_stack_trace_error_to_napi, solidity_stack_trace_heuristic_failed_to_napi,
        solidity_stack_trace_success_to_napi,
    },
};

/// A heuristic for the memory size of a [`edr_napi_core::spec::Response`]
/// object, reported as external memory to ensure GC triggers in a timely
/// manner.
///
/// When calling `Env::adjust_external_memory`, the exact same amount needs to
/// be reported for allocation and deallocation.
pub const RESPONSE_MEM_SIZE_HEURISTIC: i64 = 16 * 1_024 * 1_024;

#[napi(custom_finalize)]
pub struct Response {
    dropped_response_sender: AsyncDeallocatorSender<edr_napi_core::spec::Response>,
    inner: edr_napi_core::spec::Response,
}

impl Response {
    /// Constructs a new instance.
    pub fn new(
        response: edr_napi_core::spec::Response,
        dropped_response_sender: AsyncDeallocatorSender<edr_napi_core::spec::Response>,
    ) -> Self {
        Self {
            dropped_response_sender,
            inner: response,
        }
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

impl ObjectFinalize for Response {
    fn finalize(self, env: napi::Env) -> napi::Result<()> {
        let Self {
            dropped_response_sender,
            inner,
        } = self;

        // Off-loads deallocation of memory-heavy `call_trace_arenas` to a background
        // thread to avoid blocking the JS thread; wasting valuable time.
        dropped_response_sender.deallocate(inner);

        // Signal to the GC that the memory used by this object has been freed
        env.adjust_external_memory(-RESPONSE_MEM_SIZE_HEURISTIC)?;

        Ok(())
    }
}

/// A wrapper around [`Response`] that reports external memory usage to ensure
/// the GC triggers in a timely manner.
///
/// This is merely used during construction of the [`Response`] JS object from
/// Rust. Once constructed, the [`Response`] object itself is returned to JS,
/// and the finalizer on that object handles deallocation.
#[repr(transparent)]
pub struct GcResponse(Response);

impl From<Response> for GcResponse {
    fn from(response: Response) -> Self {
        GcResponse(response)
    }
}

impl ToNapiValue for GcResponse {
    unsafe fn to_napi_value(
        env: napi::sys::napi_env,
        val: Self,
    ) -> napi::Result<napi::sys::napi_value> {
        let env = napi::Env::from_raw(env);

        // Signal to the GC that this object holds external memory. We use a heuristic
        // instead of the actual memory size, as it's difficult to compute the exact
        // size of the `Response` object and signaling the exact size is not necessary
        // for the GC to trigger in a timely manner.
        env.adjust_external_memory(RESPONSE_MEM_SIZE_HEURISTIC)?;

        // SAFETY:
        // - the safety requirement for `env` is propagated through the `to_napi_value`
        //   function.
        // - `val.0` is a valid `Response` object.
        unsafe { Response::to_napi_value(env.raw(), val.0) }
    }
}
