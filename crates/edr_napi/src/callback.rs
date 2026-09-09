//! Keeping consumer callbacks out of threadsafe functions' roots.
//!
//! A threadsafe function holds its JavaScript function in a global handle.
//! The handle is a root V8 traces from rather than an edge it can trace to,
//! so a consumer callback handed straight to a threadsafe function is rooted
//! from outside the traced heap. If the callback references the object that
//! owns the threadsafe function, they form a cycle. A tracing collector
//! would reclaim that cycle as a unit, but one of its edges now starts at
//! the untraceable root. The cycle is therefore never collected, so the
//! object's finalizer never runs. Any native resource the finalizer releases
//! then leaks.
//!
//! Unrooting a callback builds a trampoline to give the threadsafe function
//! in its place. The trampoline forwards arguments, return value, and pending
//! exception unchanged. It reaches the callback through a reference holding
//! no count, so it keeps nothing alive. Attaching the callback to a
//! JavaScript object then decides its lifetime. V8 traces that edge, so a
//! callback referencing its owner is an ordinary collectable cycle.
//!
//! The two steps come paired, so no caller obtains a trampoline without
//! deciding who owns the callback. [`ProviderCallbacks::unroot_into`] serves
//! callbacks attached together when their owner is created, and
//! [`unroot_pending`] serves a callback whose attachment has to wait for work
//! on another thread. A call arriving after the owner's collection is handled
//! as [`OnOwnerCollected`] specifies.

use std::{ffi::CString, ptr};

use napi::{
    bindgen_prelude::{
        FromNapiValue as _, Function, FunctionCallContext, JsObjectValue as _,
        JsValuesTupleIntoVec, Object, ToNapiValue,
    },
    sys, Env, JsValue as _, Property, PropertyAttributes, Unknown,
};

/// The property under which an object owns the callbacks whose lifetime it
/// governs.
///
/// Defined non-enumerable, non-writable and non-configurable, so it stays out
/// of `Object.keys`, `console.log` and `JSON.stringify`. A consumer can
/// neither replace nor delete it. The entries inside it are ordinary
/// properties, so a callback set a second time replaces its entry rather than
/// failing.
const CALLBACKS_PROPERTY: &str = "__edrCallbacks";

/// The `Weak` flag every threadsafe function calling a consumer callback is
/// built with.
///
/// It unreferences the threadsafe function from the event loop, so holding
/// one does not keep the process running. Despite what `Weak` suggests, it
/// has nothing to do with garbage collection. The threadsafe function's root
/// is as strong as ever, and keeping the consumer's callback out of that root
/// is what this module is for.
pub const EVENT_LOOP_UNREFERENCED: bool = true;

/// What the trampoline does with a call once the callback's owner has been
/// collected.
///
/// A collected owner means the consumer let go of it. The native side may
/// still be winding down, or still serving a call that was in flight. Either
/// way a late call can arrive.
#[derive(Clone, Copy)]
pub enum OnOwnerCollected {
    /// Return `undefined`, dropping the call. For a callback whose result the
    /// caller ignores, so a late event disappears silently.
    DropCall,
    /// Throw an error naming the callback. For a callback whose result the
    /// caller consumes: `undefined` would fail its result conversion with a
    /// message pointing nowhere near the cause. Only fit for a threadsafe
    /// function invoked with `call_with_return_value`, which hands the
    /// exception to Rust. Under plain `call` the throw would escalate to an
    /// `uncaughtException`.
    Throw,
}

/// A consumer callback that no JavaScript object owns yet.
///
/// Unrooting a callback leaves it with no owner at all, so it must be attached
/// to one before anything decides when it can be collected. Until then its
/// `reference` still holds a count, which is to say the callback is not yet
/// unrooted in the strict sense. That count is needed because nothing else is
/// guaranteed to hold the callback in that window. A configuration object
/// holding it is typically a temporary. [`Self::attach_to_holder`] releases
/// the count once an object owns the callback.
///
/// This is a handle, not an owner. It carries no `Drop`, so it is safe to move
/// between threads and to drop on any of them. The trampoline's closure owns
/// the reference and deletes it when finalized.
///
/// Dropping one without attaching it is not unsound, but it restores the leak
/// this module exists to prevent. The reference keeps its count until the
/// trampoline is finalized. A callback that captures its owner therefore
/// keeps the owner alive, and with it the threadsafe function rooting the
/// trampoline. Neither is then ever reclaimed. Only when the threadsafe
/// function drops for another reason — its owner's creation failing, say — is
/// the callback reclaimed with it.
struct UnownedCallback {
    reference: sys::napi_ref,
    key: &'static str,
}

// SAFETY: `reference` is an opaque Node-API handle, never dereferenced by
// Rust. It is only passed back to Node-API from `attach_to_holder`, which runs
// on the environment's JavaScript thread. Moving the handle between threads
// before attaching touches neither the environment nor the reference.
unsafe impl Send for UnownedCallback {}

impl UnownedCallback {
    /// Makes `holder`'s object the owner of the callback, so the callback
    /// lives exactly as long as that object. Takes an already-resolved
    /// holder, so a batch shares one lookup.
    ///
    /// Attaching releases the reference's only count, so it must happen at
    /// most once. Taking `self` by value makes a second call fail to compile.
    ///
    /// # Safety
    ///
    /// `env` must be a valid environment, and the call must be on its
    /// JavaScript thread. `holder` must be this environment's callbacks holder
    /// for the owning object. The call must not come from a basic finalizer,
    /// where Node-API 10 does not permit `napi_reference_unref`.
    unsafe fn attach_to_holder(
        self,
        env: sys::napi_env,
        holder: sys::napi_value,
    ) -> napi::Result<()> {
        let mut callback = ptr::null_mut();

        // SAFETY: the caller guarantees `env` is valid and on its JavaScript
        // thread, and the reference was created on it. It still holds its
        // count, because taking `self` by value means this runs once.
        napi::check_status!(
            unsafe { sys::napi_get_reference_value(env, self.reference, &mut callback) },
            "Failed to read the `{}` callback",
            self.key
        )?;

        // A released count is the only way this reads as absent, and nothing
        // has released it yet.
        debug_assert!(
            !callback.is_null(),
            "the `{}` callback reference had already released its count",
            self.key
        );

        let key = CString::new(self.key)?;

        // SAFETY: the caller guarantees `env` is valid and on its JavaScript
        // thread, and that `holder` is a live object in it. `callback` is the
        // live function just read from the reference.
        napi::check_status!(
            unsafe { sys::napi_set_named_property(env, holder, key.as_ptr(), callback) },
            "Failed to hand the `{}` callback to its owner",
            self.key
        )?;

        let mut count = 0;

        // The owner now holds the callback, so releasing the count leaves it
        // reachable for exactly as long as the owner is.
        //
        // SAFETY: the caller guarantees `env` is valid, on its JavaScript
        // thread, and not inside a basic finalizer, where Node-API 10 does
        // not permit `napi_reference_unref`. The reference was created on
        // `env` and still holds its count.
        napi::check_status!(
            unsafe { sys::napi_reference_unref(env, self.reference, &mut count) },
            "Failed to release the `{}` callback reference's count",
            self.key
        )?;

        debug_assert_eq!(
            count, 0,
            "the `{}` callback reference still holds a count",
            self.key
        );

        Ok(())
    }
}

/// Reads `owner`'s callbacks holder, creating it if this is its first
/// callback.
///
/// # Safety
///
/// `env` must be a valid environment, the call must be on its JavaScript
/// thread, and `owner` must be a live object in it.
unsafe fn callbacks_holder(
    env: sys::napi_env,
    owner: sys::napi_value,
) -> napi::Result<sys::napi_value> {
    let mut key = ptr::null_mut();

    // SAFETY: the caller guarantees `env` is valid and on its JavaScript
    // thread. The name is a `&'static str`, so the pointer and length match.
    napi::check_status!(
        unsafe {
            sys::napi_create_string_utf8(
                env,
                CALLBACKS_PROPERTY.as_ptr().cast(),
                CALLBACKS_PROPERTY.len() as isize,
                &mut key,
            )
        },
        "Failed to create the callbacks property name"
    )?;

    let mut has_holder = false;

    // Own property rather than a lookup through the prototype. The holder
    // belongs to this object, so a holder on a shared prototype would hand
    // every instance the same callbacks.
    //
    // SAFETY: the caller guarantees `env` is valid and on its JavaScript
    // thread, and that `owner` is a live object in it. `key` is the string
    // just created in `env`.
    napi::check_status!(
        unsafe { sys::napi_has_own_property(env, owner, key, &mut has_holder) },
        "Failed to look up the callbacks holder"
    )?;

    if has_holder {
        let mut holder = ptr::null_mut();

        // SAFETY: the caller guarantees `env` is valid and on its JavaScript
        // thread, and that `owner` is a live object in it. `key` is the
        // string just created in `env`, and the property exists.
        napi::check_status!(
            unsafe { sys::napi_get_property(env, owner, key, &mut holder) },
            "Failed to read the callbacks holder"
        )?;

        return Ok(holder);
    }

    let mut holder = ptr::null_mut();

    // SAFETY: the caller guarantees `env` is valid and on its JavaScript
    // thread.
    napi::check_status!(
        unsafe { sys::napi_create_object(env, &mut holder) },
        "Failed to create the callbacks holder"
    )?;

    // SAFETY: the caller guarantees `env` is valid and on its JavaScript
    // thread, and that `owner` is a live object in it.
    let mut owner = unsafe { Object::from_napi_value(env, owner)? };

    // SAFETY: the caller guarantees `env` is valid and on its JavaScript
    // thread. `holder` is the object just created in it.
    let holder_value = unsafe { Unknown::from_raw_unchecked(env, holder) };

    // `PropertyAttributes::Default` is Node-API's `napi_default`: no
    // attributes at all, so non-enumerable, non-configurable and non-writable.
    // `Property::new` starts out writable, enumerable and configurable, so
    // this has to be set.
    owner.define_properties(&[Property::new()
        .with_utf8_name(CALLBACKS_PROPERTY)?
        .with_value(&holder_value)
        .with_property_attributes(PropertyAttributes::Default)])?;

    Ok(holder)
}

/// What [`unroot`] produces.
struct Unrooted<'env, Args: JsValuesTupleIntoVec, Return> {
    /// Hand this to `build_threadsafe_function` in place of the consumer's
    /// callback, so the threadsafe function roots the trampoline instead.
    trampoline: Function<'env, Args, Return>,
    /// Attach to the object whose lifetime the callback should share.
    unowned: UnownedCallback,
}

/// Stops the threadsafe function built from the returned trampoline from being
/// a root for `callback`.
///
/// `key` names the callback under its owner, in its error messages, and as
/// the trampoline's own function name, so a stack trace taken inside a
/// consumer's callback says which one it is. It must be unique among the
/// callbacks one object owns. A shared name would silently overwrite the
/// earlier entry, orphaning its callback.
///
/// The returned [`Unrooted::unowned`] has to be attached to an object, or the
/// callback stays rooted. That is the leak this exists to prevent. The public
/// entry points, [`ProviderCallbacks::unroot_into`] and [`unroot_pending`],
/// each pair the two steps.
fn unroot<'env, Args, Return>(
    env: &'env Env,
    callback: Function<'env, Args, Return>,
    key: &'static str,
    owner_collected: OnOwnerCollected,
) -> napi::Result<Unrooted<'env, Args, Return>>
where
    Args: JsValuesTupleIntoVec,
{
    // Deletes the reference when the trampoline that owns it is finalized.
    // Defined inside `unroot` so nothing else can hold one. Node-API runs a
    // function's finalizer on the environment's JavaScript thread, so the
    // deletion is always on the right thread. The reference therefore
    // outlives both the owning object and the threadsafe function, so the
    // trampoline can never read a deleted reference. `napi_delete_reference`
    // takes a `node_api_basic_env`, so a basic finalizer may call it. This
    // therefore stays valid under Node-API 10, where a non-permitted call
    // aborts the process.
    struct ReferenceOwner {
        env: sys::napi_env,
        reference: sys::napi_ref,
    }

    impl ReferenceOwner {
        // The reference this owns. Deliberately a method rather than a field
        // read. Rust 2021 `move` closures capture disjoint fields, and
        // `sys::napi_ref` is `Copy`. A closure naming only the `reference`
        // field would therefore copy that field and leave the owner behind.
        // The owner would then drop at the end of `unroot`, deleting the
        // reference while the trampoline still expects to read it. Going
        // through `&self` captures the owner instead.
        fn reference(&self) -> sys::napi_ref {
            self.reference
        }
    }

    impl Drop for ReferenceOwner {
        fn drop(&mut self) {
            // SAFETY: `env` and `reference` come from `unroot`, which creates
            // the reference on that environment. `Drop` runs from the
            // trampoline's finalizer, on the environment's JavaScript thread,
            // and runs once because only the trampoline's closure owns this.
            let status = unsafe { sys::napi_delete_reference(self.env, self.reference) };

            // Nothing can be done about a failure here, and the environment
            // is being torn down in the case that produces one.
            debug_assert!(
                status == sys::Status::napi_ok,
                "failed to delete a callback reference: {}",
                napi::Status::from(status)
            );
        }
    }

    let mut reference = ptr::null_mut();

    // Created with a count, so the callback survives until it is attached to
    // an owner.
    //
    // SAFETY: `env` is a live environment on its JavaScript thread, as holding
    // an `&Env` implies, and `callback` is a function in it.
    napi::check_status!(
        unsafe { sys::napi_create_reference(env.raw(), callback.raw(), 1, &mut reference) },
        "Failed to reference the `{}` callback",
        key
    )?;

    // The trampoline is what the threadsafe function roots. It owns the
    // reference — and so deletes it, on this thread, when it is finalized —
    // but the reference holds no count once the callback is attached, so
    // rooting the trampoline roots nothing else.
    let owner = ReferenceOwner {
        env: env.raw(),
        reference,
    };

    let trampoline = env.create_function_from_closure::<Args, sys::napi_value, _>(
        key,
        move |ctx: FunctionCallContext<'_>| {
            let env = ctx.env.raw();

            let mut target = ptr::null_mut();

            // SAFETY: the reference was created on this environment, which is
            // the one calling the trampoline, and `owner` keeps it alive for
            // as long as this closure exists.
            napi::check_status!(
                unsafe { sys::napi_get_reference_value(env, owner.reference(), &mut target) },
                "Failed to read an unrooted callback"
            )?;

            let mut undefined = ptr::null_mut();

            // SAFETY: `env` is the live environment calling this trampoline.
            napi::check_status!(
                unsafe { sys::napi_get_undefined(env, &mut undefined) },
                "Failed to get undefined"
            )?;

            if target.is_null() {
                // The object owning the callback has been collected, so there
                // is no longer anyone to deliver this call to.
                return match owner_collected {
                    OnOwnerCollected::DropCall => Ok(undefined),
                    OnOwnerCollected::Throw => Err(napi::Error::new(
                        napi::Status::GenericFailure,
                        format!("The provider owning the `{key}` callback has been collected"),
                    )),
                };
            }

            // Forwarded unchanged, so the consumer's callback sees exactly the
            // call the threadsafe function made.
            let arguments = (0..ctx.length())
                .map(|index| ctx.get::<Unknown<'_>>(index).map(|argument| argument.raw()))
                .collect::<napi::Result<Vec<_>>>()?;

            let mut result = ptr::null_mut();

            // A throw from the consumer's callback stays pending.
            // `check_status!` reports it without clearing it, and napi
            // rethrows a `PendingException` error by leaving the pending
            // exception in place. The threadsafe function therefore sees the
            // exception it would have seen calling the callback itself, with
            // the thrown value untouched. `check_pending_exception!` would
            // clear and coerce it instead, destroying a thrown primitive.
            //
            // SAFETY: `env` is the live environment calling this trampoline.
            // `target` is the live function just read from the reference, and
            // `arguments` are live values from this call.
            napi::check_status!(
                unsafe {
                    sys::napi_call_function(
                        env,
                        undefined,
                        target,
                        arguments.len(),
                        arguments.as_ptr(),
                        &mut result,
                    )
                },
                "Failed to call an unrooted callback"
            )?;

            Ok(result)
        },
    )?;

    // The trampoline forwards its arguments and its return value unchanged, so
    // it accepts and returns whatever the consumer's callback does. `Function`
    // carries those only as type parameters, so this re-typing describes the
    // callback behind the trampoline rather than changing anything.
    //
    // SAFETY: `env` is live and `trampoline` is the function just created in
    // it.
    let trampoline =
        unsafe { Function::<'env, Args, Return>::from_napi_value(env.raw(), trampoline.raw())? };

    Ok(Unrooted {
        trampoline,
        unowned: UnownedCallback { reference, key },
    })
}

/// The callbacks a provider's JavaScript object owns, registered as the
/// provider's configuration is resolved.
///
/// The call override is not here. It is set on a provider that already
/// exists, so it is attached through a [`PendingAttachment`] instead.
#[derive(Default)]
pub struct ProviderCallbacks(Vec<UnownedCallback>);

impl ProviderCallbacks {
    /// Unroots `callback` and registers it to be attached to the provider's
    /// JavaScript object. `build` receives the trampoline and builds the
    /// threadsafe function from it, so the trampoline never escapes this
    /// module.
    ///
    /// Registering here is what makes [`ProviderWithCallbacks`] attach the
    /// callback, so this is the unrooting path for every callback a provider
    /// is configured with. `key` names the callback under its owner and must
    /// be unique among the callbacks one object owns.
    pub fn unroot_into<'env, Args, Return, BuiltT>(
        &mut self,
        env: &'env Env,
        callback: Function<'env, Args, Return>,
        key: &'static str,
        owner_collected: OnOwnerCollected,
        build: impl FnOnce(Function<'env, Args, Return>) -> napi::Result<BuiltT>,
    ) -> napi::Result<BuiltT>
    where
        Args: JsValuesTupleIntoVec,
    {
        let Unrooted {
            trampoline,
            unowned,
        } = unroot(env, callback, key, owner_collected)?;

        // A failure in `build` drops `unowned` before it is registered. Any
        // threadsafe function built from the trampoline drops with the error,
        // so the trampoline's finalizer reclaims the callback.
        let built = build(trampoline)?;

        self.0.push(unowned);

        Ok(built)
    }

    /// Makes `owner` the owner of every registered callback.
    ///
    /// # Safety
    ///
    /// `env` must be a valid environment, the call must be on its JavaScript
    /// thread, and `owner` must be a live object in it. The call must not
    /// come from a basic finalizer, where Node-API 10 does not permit
    /// `napi_reference_unref`.
    unsafe fn attach_to(self, env: sys::napi_env, owner: sys::napi_value) -> napi::Result<()> {
        // Two callbacks sharing a key would silently overwrite one another's
        // entry, orphaning the earlier callback.
        debug_assert!(
            self.0.iter().enumerate().all(|(index, callback)| {
                self.0
                    .iter()
                    .take(index)
                    .all(|earlier| earlier.key != callback.key)
            }),
            "two registered callbacks share a key"
        );

        // SAFETY: the caller guarantees `env` is valid and on its JavaScript
        // thread, and that `owner` is a live object in it.
        let holder = unsafe { callbacks_holder(env, owner)? };

        for callback in self.0 {
            // SAFETY: the caller guarantees `env` is valid, on its JavaScript
            // thread, and not inside a basic finalizer. `holder` is `owner`'s
            // callbacks holder, just read or created in `env`.
            unsafe { callback.attach_to_holder(env, holder)? };
        }

        Ok(())
    }
}

/// Who will own a callback handed to a threadsafe function.
///
/// The choice of owner lives in the type rather than in a runtime variant,
/// because every call site knows its owner at compile time. The implementor
/// decides whether the threadsafe function receives the callback itself or a
/// trampoline for it.
pub trait CallbackOwner {
    /// Settles who owns `callback`, then builds the threadsafe function.
    ///
    /// `build` receives either the callback itself or a trampoline for it,
    /// so a trampoline never escapes.
    fn own<'env, Args, Return, BuiltT>(
        &mut self,
        env: &'env Env,
        callback: Function<'env, Args, Return>,
        key: &'static str,
        owner_collected: OnOwnerCollected,
        build: impl FnOnce(Function<'env, Args, Return>) -> napi::Result<BuiltT>,
    ) -> napi::Result<BuiltT>
    where
        Args: JsValuesTupleIntoVec;
}

/// A provider's JavaScript object will own the callback, so the threadsafe
/// function must not root it. The callback is registered here, and the
/// threadsafe function is given its trampoline instead. See the module
/// documentation for the leak this prevents.
impl CallbackOwner for ProviderCallbacks {
    fn own<'env, Args, Return, BuiltT>(
        &mut self,
        env: &'env Env,
        callback: Function<'env, Args, Return>,
        key: &'static str,
        owner_collected: OnOwnerCollected,
        build: impl FnOnce(Function<'env, Args, Return>) -> napi::Result<BuiltT>,
    ) -> napi::Result<BuiltT>
    where
        Args: JsValuesTupleIntoVec,
    {
        self.unroot_into(env, callback, key, owner_collected, build)
    }
}

/// Roots the callback in the threadsafe function itself.
///
/// For a threadsafe function whose own lifetime is already bounded, so
/// rooting the callback cannot outlive that bound. It is handed the callback
/// directly, with no trampoline and no reference to manage.
pub struct RootedByThreadsafeFunction;

impl CallbackOwner for RootedByThreadsafeFunction {
    fn own<'env, Args, Return, BuiltT>(
        &mut self,
        _env: &'env Env,
        callback: Function<'env, Args, Return>,
        _key: &'static str,
        _owner_collected: OnOwnerCollected,
        build: impl FnOnce(Function<'env, Args, Return>) -> napi::Result<BuiltT>,
    ) -> napi::Result<BuiltT>
    where
        Args: JsValuesTupleIntoVec,
    {
        build(callback)
    }
}

/// What [`unroot_pending`] produces.
pub struct PendingUnroot<BuiltT> {
    /// What `build` made from the trampoline.
    pub built: BuiltT,
    /// Completes or abandons the attachment on the JavaScript thread.
    pub attachment: PendingAttachment,
}

/// Unroots `callback` and prepares attaching it to `owner`, for an attachment
/// that has to wait for work on another thread.
///
/// `build` receives the trampoline and builds the threadsafe function from
/// it, so the trampoline never escapes this module. Taking `build` as a
/// closure also keeps a `?` at the call site from dropping a formed
/// attachment, because the owner reference is only created once every
/// fallible step has succeeded. `key` names the callback under its owner and
/// must be unique among the callbacks one object owns.
pub fn unroot_pending<'env, Args, Return, BuiltT>(
    env: &'env Env,
    owner: &Object<'_>,
    callback: Function<'env, Args, Return>,
    key: &'static str,
    owner_collected: OnOwnerCollected,
    build: impl FnOnce(Function<'env, Args, Return>) -> napi::Result<BuiltT>,
) -> napi::Result<PendingUnroot<BuiltT>>
where
    Args: JsValuesTupleIntoVec,
{
    let Unrooted {
        trampoline,
        unowned,
    } = unroot(env, callback, key, owner_collected)?;

    // A failure here or below drops `unowned`. Any threadsafe function built
    // from the trampoline drops with the error, so the trampoline's finalizer
    // reclaims the callback.
    let built = build(trampoline)?;

    let attachment = PendingAttachment::new(env, owner, unowned)?;

    Ok(PendingUnroot { built, attachment })
}

/// A callback together with the owner it will be attached to, for an
/// attachment that has to wait for work on another thread.
///
/// A callback that replaces a predecessor needs this. The entry it takes is
/// the predecessor's only owner. The new callback must therefore take that
/// entry only once nothing calls the predecessor anymore. The work that
/// stops those calls happens on another thread, so the attachment completes
/// afterwards in a JavaScript-thread closure. The owner is held through a
/// counted reference. The callback's own reference keeps its count until
/// attachment, so both survive the hand-off. Attaching releases both counts.
/// Abandoning releases only the owner's, and the callback's then lasts until
/// the trampoline is finalized.
///
/// Dropping one without calling [`Self::attach`] or [`Self::abandon`] leaks.
/// The owner reference keeps its count, so the owner is rooted for the
/// environment's life. In practice only environment teardown drops one that
/// way. No `Drop` impl could help. That drop can happen off the JavaScript
/// thread, where deleting the reference is not allowed.
pub struct PendingAttachment {
    owner: sys::napi_ref,
    callback: UnownedCallback,
}

// SAFETY: both fields hold opaque Node-API handles, never dereferenced by
// Rust and only passed back to Node-API from `attach` and `abandon`, which
// run on the environment's JavaScript thread.
unsafe impl Send for PendingAttachment {}

impl PendingAttachment {
    /// Prepares attaching `callback` to `owner`.
    fn new(env: &Env, owner: &Object<'_>, callback: UnownedCallback) -> napi::Result<Self> {
        let mut reference = ptr::null_mut();

        // Created with a count, so the owner outlives the hand-off.
        //
        // SAFETY: `env` is a live environment on its JavaScript thread, as
        // holding an `&Env` implies, and `owner` is a live object in it.
        napi::check_status!(
            unsafe { sys::napi_create_reference(env.raw(), owner.raw(), 1, &mut reference) },
            "Failed to reference the `{}` callback's owner",
            callback.key
        )?;

        Ok(Self {
            owner: reference,
            callback,
        })
    }

    /// Completes the attachment, making the owner own the callback.
    ///
    /// A failure leaves the callback unowned while it can still be called. A
    /// callback capturing its owner then keeps the owner alive for the
    /// environment's life. In practice only environment teardown produces a
    /// failure. Must not be called from a basic finalizer, where Node-API 10
    /// permits almost none of the calls this makes.
    pub fn attach(self, env: &Env) -> napi::Result<()> {
        let Self { owner, callback } = self;

        let mut object = ptr::null_mut();

        // SAFETY: holding an `&Env` implies being on its JavaScript thread,
        // and the reference was created on it in `new`.
        let read = napi::check_status!(
            unsafe { sys::napi_get_reference_value(env.raw(), owner, &mut object) },
            "Failed to read the `{}` callback's owner",
            callback.key
        );

        // The owner reference has served its purpose whether or not the read
        // succeeded.
        //
        // SAFETY: holding an `&Env` implies being on its JavaScript thread,
        // and the reference was created on it in `new`. Consuming `self`
        // means this runs once.
        let deleted = unsafe { sys::napi_delete_reference(env.raw(), owner) };
        debug_assert!(
            deleted == sys::Status::napi_ok,
            "failed to delete an owner reference: {}",
            napi::Status::from(deleted)
        );

        read?;

        // The reference held a count, so the owner cannot have been collected.
        debug_assert!(
            !object.is_null(),
            "the `{}` callback's owner reference read as absent",
            callback.key
        );

        // SAFETY: holding an `&Env` implies a valid environment on its
        // JavaScript thread, and `object` is the live owner just read.
        let holder = unsafe { callbacks_holder(env.raw(), object)? };

        // SAFETY: holding an `&Env` implies a valid environment on its
        // JavaScript thread, and `holder` is the owner's callbacks holder,
        // just read or created. This method's documentation forbids calling
        // it from a basic finalizer.
        unsafe { callback.attach_to_holder(env.raw(), holder) }
    }

    /// Abandons the attachment, releasing the owner reference without giving
    /// the owner the callback.
    ///
    /// For the path where the callback's installation failed, so nothing will
    /// call it. Its threadsafe function drops with the failure, and the
    /// trampoline's finalizer then reclaims the callback.
    pub fn abandon(self, env: &Env) {
        // SAFETY: holding an `&Env` implies being on its JavaScript thread,
        // and the reference was created on it in `new`. Consuming `self`
        // means this runs once.
        let status = unsafe { sys::napi_delete_reference(env.raw(), self.owner) };

        debug_assert!(
            status == sys::Status::napi_ok,
            "failed to delete an owner reference: {}",
            napi::Status::from(status)
        );
    }
}

/// A provider on its way to JavaScript, whose object takes ownership of the
/// consumer callbacks it was configured with as it is created.
///
/// See the module documentation for why the callbacks are owned here rather
/// than by the threadsafe functions that call them.
pub struct ProviderWithCallbacks<ProviderT> {
    provider: ProviderT,
    callbacks: ProviderCallbacks,
}

impl<ProviderT> ProviderWithCallbacks<ProviderT> {
    pub fn new(provider: ProviderT, callbacks: ProviderCallbacks) -> Self {
        Self {
            provider,
            callbacks,
        }
    }
}

impl<ProviderT: ToNapiValue> ToNapiValue for ProviderWithCallbacks<ProviderT> {
    unsafe fn to_napi_value(env: sys::napi_env, val: Self) -> napi::Result<sys::napi_value> {
        // SAFETY: `env` is valid, as this function's own contract requires.
        let provider = unsafe { ProviderT::to_napi_value(env, val.provider)? };

        // SAFETY: `env` is valid and on its JavaScript thread, as this
        // function's contract requires, and `provider` is the object just
        // built in it. `to_napi_value` runs during a value conversion, never
        // from a finalizer. A `ProviderWithCallbacks` is consumed here, and
        // one is built per provider, so each callback is attached once.
        unsafe { val.callbacks.attach_to(env, provider)? };

        Ok(provider)
    }
}
