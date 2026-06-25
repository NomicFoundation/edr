//! Variants that grow a bare napi-rs v3 addon toward EDR's shape, ~2 new
//! variables at a time, hunting the `Check failed: node->IsInUse()` teardown
//! double-free (nodejs/node#52418). The plain weak TSFN (`weak_tsfn`) is 0/500
//! across musl/gnu × node 24/26 — so the trigger is something below.
//!
//! Each `#[napi]` export is one mode; `test.js <mode>` runs it, looped in CI.
//! Suspect variables: ObjectWrap `Reference` (a `#[napi]` class instance), a
//! TSFN that is actually *called*, and entering a tokio runtime.

use std::thread;

use napi::bindgen_prelude::{Function, ObjectFinalize};
use napi::threadsafe_function::{
    ThreadsafeCallContext, ThreadsafeFunction, ThreadsafeFunctionCallMode,
};
use napi::{Env, Result};
use napi_derive::napi;

type WeakTsfn = ThreadsafeFunction<u32, (), u32, napi::Status, false, true, 0>;

fn build_weak_tsfn(callback: Function<u32, ()>) -> Result<WeakTsfn> {
    callback
        .build_threadsafe_function::<u32>()
        .weak::<true>()
        .build_callback(|ctx: ThreadsafeCallContext<u32>| Ok(ctx.value))
}

/// CONTROL (V0): weak TSFN, leaked, never called. Known 0/500.
#[napi]
pub fn weak_tsfn(callback: Function<u32, ()>) -> Result<()> {
    std::mem::forget(build_weak_tsfn(callback)?);
    Ok(())
}

/// V1 (prime suspect, single): an ObjectWrap `#[napi]` class instance — holds a
/// napi `Reference` like EdrContext/Provider/ContractDecoder. Created + GC'd.
#[napi]
pub struct PlainWrap {
    _value: u32,
}

#[napi]
impl PlainWrap {
    #[napi(constructor)]
    pub fn new() -> Self {
        PlainWrap { _value: 0 }
    }
}

/// V1 + V4: ObjectWrap holding a weak TSFN (Provider-holds-callbacks shape).
#[napi]
pub struct WrapHoldingTsfn {
    _tsfn: WeakTsfn,
}

#[napi]
impl WrapHoldingTsfn {
    #[napi(constructor)]
    pub fn new(callback: Function<u32, ()>) -> Result<Self> {
        Ok(WrapHoldingTsfn {
            _tsfn: build_weak_tsfn(callback)?,
        })
    }
}

/// V1 + V5: ObjectWrap whose custom finalize offloads its own drop to a
/// background thread (Provider::finalize + AsyncDeallocator shape) — releases
/// the held weak-TSFN handle off the JS thread, racing teardown.
#[napi(custom_finalize)]
pub struct WrapOffThreadDrop {
    tsfn: Option<WeakTsfn>,
}

#[napi]
impl WrapOffThreadDrop {
    #[napi(constructor)]
    pub fn new(callback: Function<u32, ()>) -> Result<Self> {
        Ok(WrapOffThreadDrop {
            tsfn: Some(build_weak_tsfn(callback)?),
        })
    }
}

impl ObjectFinalize for WrapOffThreadDrop {
    fn finalize(mut self, _env: Env) -> Result<()> {
        let tsfn = self.tsfn.take();
        thread::spawn(move || drop(tsfn));
        Ok(())
    }
}

/// V2 (single): weak TSFN that is actually *called* once before exit.
#[napi]
pub fn called_weak_tsfn(callback: Function<u32, ()>) -> Result<()> {
    let tsfn = build_weak_tsfn(callback)?;
    tsfn.call(0, ThreadsafeFunctionCallMode::NonBlocking);
    std::mem::forget(tsfn);
    Ok(())
}

/// V2 + V3: weak TSFN called from a tokio runtime task (subscription-from-
/// runtime shape). `async_runtime` makes napi enter the runtime for this sync
/// entry point, mirroring EDR's EdrContext methods.
#[napi(async_runtime)]
pub fn runtime_called_tsfn(callback: Function<u32, ()>) -> Result<()> {
    let tsfn = build_weak_tsfn(callback)?;
    napi::tokio::spawn(async move {
        tsfn.call(0, ThreadsafeFunctionCallMode::NonBlocking);
        std::mem::forget(tsfn);
    });
    Ok(())
}
