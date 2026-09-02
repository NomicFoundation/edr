use napi::{bindgen_prelude::ToNapiValue, sys, Env};

/// Declares how much external memory a type's JavaScript object stands for.
///
/// Implemented by `gc_tracked!`, which also emits the finalizer releasing
/// [`Self::EXTERNAL_MEMORY`] again. Implementing it by hand gets the report
/// without that release.
#[diagnostic::on_unimplemented(
    message = "`{Self}` is not declared with `gc_tracked!`",
    label = "not declared with `gc_tracked!`",
    note = "invoke `gc_tracked!`, which declares the alias, the amount and the finalizer together"
)]
pub trait HasExternalMemory {
    /// Bytes reported to V8 while the JavaScript object is alive.
    const EXTERNAL_MEMORY: i64;
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
/// [`HasExternalMemory::EXTERNAL_MEMORY`] bytes to V8.
///
/// `gc_tracked!` is the only thing that implements [`HasExternalMemory`], and
/// it emits the finalizer that releases the same constant. The report and its
/// release are therefore declared together, and neither can name a different
/// amount.
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
        Env::from_raw(env).adjust_external_memory(T::EXTERNAL_MEMORY)?;

        // SAFETY: `env` is valid, as this function's own contract requires.
        unsafe { T::to_napi_value(env, val.0) }
    }
}

/// Declares a [`GcTracked`] alias for a type, along with the amount its
/// JavaScript object reports and the finalizer that releases it.
///
/// `GcTracked` in the alias is a literal token rather than a path, so it needs
/// no import. `fn drop` takes `self` by value, and `mut self` is not
/// supported.
///
/// ```text
/// gc_tracked! {
///     /// A `Provider` on its way to JavaScript.
///     pub(crate) type GcProvider = GcTracked<Provider>;
///
///     /// What the JavaScript object stands for.
///     const EXTERNAL_MEMORY: i64 = 2 * 1024 * 1024;
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
        const EXTERNAL_MEMORY: i64 = $external_memory:expr;

        fn drop($self:ident) $body:block
    ) => {
        $(#[$alias_meta])*
        $alias_vis type $alias = $crate::gc::GcTracked<$ty>;

        impl $crate::gc::HasExternalMemory for $ty {
            $(#[$memory_meta])*
            const EXTERNAL_MEMORY: i64 = $external_memory;
        }

        impl $crate::gc::MoveTheDropImplIntoGcTracked for $ty {}

        impl $ty {
            fn __gc_tracked_drop($self) $body
        }

        impl ::napi::bindgen_prelude::ObjectFinalize for $ty {
            fn finalize($self, env: ::napi::Env) -> ::napi::Result<()> {
                // Runs first, so returning below cannot drop the value on the
                // JS thread.
                Self::__gc_tracked_drop($self);

                // Releases what the conversion to JavaScript reported. Only a
                // JS object reaches this finalizer, so the amounts pair up.
                env.adjust_external_memory(
                    -<Self as $crate::gc::HasExternalMemory>::EXTERNAL_MEMORY,
                )?;

                Ok(())
            }
        }
    };
}

pub(crate) use gc_tracked;
