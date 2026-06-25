use napi::bindgen_prelude::Function;
use napi::threadsafe_function::ThreadsafeCallContext;
use napi::Result;
use napi_derive::napi;

/// Mirrors EDR's weak ThreadsafeFunctions: build a `weak::<true>` TSFN and leak
/// it so the weak global handle is still registered at env teardown — the
/// condition that triggers `Check failed: node->IsInUse()` in EDR. If this bare
/// addon crashes at process exit on arm64-musl, the bug is napi-rs v3 / Node
/// (#52418), not EDR.
#[napi]
pub fn register_weak_tsfn(callback: Function<u32, ()>) -> Result<()> {
    let tsfn = callback
        .build_threadsafe_function::<u32>()
        .weak::<true>()
        .build_callback(|ctx: ThreadsafeCallContext<u32>| Ok(ctx.value))?;

    std::mem::forget(tsfn);

    Ok(())
}
