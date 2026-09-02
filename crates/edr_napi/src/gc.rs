use napi::{bindgen_prelude::ToNapiValue, sys, Env};

/// Wraps a value so that handing it to JavaScript reports `EXTERNAL_MEMORY`
/// bytes to V8.
///
/// # `T`'s finalizer must release the same amount
///
/// The wrapper exists only for the conversion. JavaScript receives `T`'s own
/// object, so `T`'s [`ObjectFinalize`] implementation is the only place that
/// can call [`Env::adjust_external_memory`] with `-EXTERNAL_MEMORY`. Nothing
/// checks that it does, so keep the two sites with the constant they share.
///
/// # Why report at all
///
/// V8 schedules collection from the pressure it can see, and a JS wrapper
/// around an expensive Rust resource is a handful of bytes. Reporting the
/// resource's weight is what makes an unreachable wrapper worth collecting.
/// If an exact size is not known, a heuristic suffices: telling Node the
/// order of magnitude of the external allocation matters more than being
/// precise.
///
/// [`ObjectFinalize`]: napi::bindgen_prelude::ObjectFinalize
#[repr(transparent)]
pub struct GcTracked<T, const EXTERNAL_MEMORY: i64>(T);

impl<T, const EXTERNAL_MEMORY: i64> From<T> for GcTracked<T, EXTERNAL_MEMORY> {
    fn from(value: T) -> Self {
        Self(value)
    }
}

impl<T: ToNapiValue, const EXTERNAL_MEMORY: i64> ToNapiValue for GcTracked<T, EXTERNAL_MEMORY> {
    unsafe fn to_napi_value(env: sys::napi_env, val: Self) -> napi::Result<sys::napi_value> {
        // Reported before the conversion, because only a JS object gets a
        // finalizer to release it. A conversion that fails afterwards leaves an
        // over-report, which makes V8 more eager rather than less.
        Env::from_raw(env).adjust_external_memory(EXTERNAL_MEMORY)?;

        // SAFETY: `env` is valid, as this function's own contract requires.
        unsafe { T::to_napi_value(env, val.0) }
    }
}
