use napi::{bindgen_prelude::ToNapiValue, sys, Env};

/// Declares how much external memory a type's JavaScript object stands for.
///
/// Implemented by `gc_tracked!`, which also emits the finalizer releasing the
/// same amount. Implementing it by hand gets the report without that release.
///
/// Must return the same value for the lifetime of the JavaScript object. A
/// type whose answer varies has to store it, so both ends read one field.
///
/// The function should run in constant time, as it can be called on a hot
/// code path! The figure is only a heuristic for the order of magnitude of
/// the externally allocated memory.
#[diagnostic::on_unimplemented(
    message = "`{Self}` is not declared with `gc_tracked!`",
    label = "not declared with `gc_tracked!`",
    note = "invoke `gc_tracked!`, which declares the alias, the amount and the finalizer together"
)]
pub trait HasExternalMemory {
    /// Bytes reported to V8 while the JavaScript object is alive.
    fn external_memory(&self) -> i64;
}

/// Blanket-implemented for every type that has its own [`Drop`], so
/// `gc_tracked!`'s matching implementation collides with it.
///
/// The collision is the diagnostic: a type reaching `gc_tracked!` with a
/// hand-written `Drop` gets an `E0119` naming this trait, whose name says
/// where that implementation belongs instead.
// Never a bound, so the collision is the only thing this trait is for.
#[expect(dead_code)]
pub trait MoveTheDropImplIntoGcTracked {}

// The `drop_bounds` lint targets exactly this bound, which is the detector
// here.
#[expect(drop_bounds)]
impl<T: Drop> MoveTheDropImplIntoGcTracked for T {}

/// Wraps a value so that handing it to JavaScript reports
/// [`HasExternalMemory::external_memory`] bytes to V8.
///
/// `gc_tracked!` is the only thing that implements [`HasExternalMemory`], and
/// it emits the finalizer that releases the same amount. The report and its
/// release are therefore declared together, and neither can name a different
/// figure.
///
/// # Why report at all
///
/// V8 schedules collection from the pressure it can see, and a JS wrapper
/// around an expensive Rust resource is a handful of bytes. Reporting the
/// resource's weight is what makes an unreachable wrapper worth collecting.
/// If an exact size is not known, a heuristic suffices: telling Node the
/// order of magnitude of the external allocation matters more than being
/// precise.
#[repr(transparent)]
pub struct GcTracked<T: HasExternalMemory>(T);

impl<T: HasExternalMemory> From<T> for GcTracked<T> {
    fn from(value: T) -> Self {
        Self(value)
    }
}

impl<T: ToNapiValue + HasExternalMemory> ToNapiValue for GcTracked<T> {
    unsafe fn to_napi_value(env: sys::napi_env, val: Self) -> napi::Result<sys::napi_value> {
        // Reported before the conversion, because only a JS object gets a
        // finalizer to release it. A conversion that fails afterwards leaves an
        // over-report, which makes V8 more eager rather than less.
        Env::from_raw(env).adjust_external_memory(val.0.external_memory())?;

        // SAFETY: `env` is valid, as this function's own contract requires.
        unsafe { T::to_napi_value(env, val.0) }
    }
}

/// Declares a [`GcTracked`] alias for a type, along with the amount its
/// JavaScript object reports and the finalizer that releases it.
///
/// `GcTracked` in the alias is a literal token rather than a path, so it needs
/// no import. Both functions take `self` by the name written here, and
/// `mut self` is not supported.
///
/// `external_memory` should run in constant time; see [`HasExternalMemory`].
///
/// ```text
/// gc_tracked! {
///     /// A `Provider` on its way to JavaScript.
///     pub(crate) type GcProvider = GcTracked<Provider>;
///
///     /// What the JavaScript object stands for.
///     fn external_memory(&self) -> i64 {
///         2 * 1024 * 1024
///     }
///
///     fn drop(self) {
///         // whatever `Drop` would have done
///     }
/// }
/// ```
macro_rules! gc_tracked {
    (
        $(#[$alias_meta:meta])*
        $alias_vis:vis type $alias:ident = GcTracked<$ty:ty>;

        $(#[$memory_meta:meta])*
        fn external_memory(&$memory_self:ident) -> i64 $memory_body:block

        fn drop($self:ident) $body:block
    ) => {
        $(#[$alias_meta])*
        $alias_vis type $alias = $crate::gc::GcTracked<$ty>;

        impl $crate::gc::HasExternalMemory for $ty {
            $(#[$memory_meta])*
            fn external_memory(&$memory_self) -> i64 $memory_body
        }

        impl $crate::gc::MoveTheDropImplIntoGcTracked for $ty {}

        impl $ty {
            fn __gc_tracked_drop($self) $body
        }

        impl ::napi::bindgen_prelude::ObjectFinalize for $ty {
            fn finalize($self, env: ::napi::Env) -> ::napi::Result<()> {
                let external_memory =
                    <Self as $crate::gc::HasExternalMemory>::external_memory(&$self);

                // Runs before the report is released, so returning below cannot
                // drop the value on the JS thread.
                Self::__gc_tracked_drop($self);

                // Releases what the conversion to JavaScript reported. Only a
                // JS object reaches this finalizer, so the amounts pair up.
                env.adjust_external_memory(-external_memory)?;

                Ok(())
            }
        }
    };
}

pub(crate) use gc_tracked;
