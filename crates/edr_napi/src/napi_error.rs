/// Extracts the error's message, deliberately leaking the error itself.
///
/// A `napi::Error` produced from a JS exception or promise rejection owns a
/// `napi_ref` to the JS error object, and its `Drop` deletes that reference
/// on the current thread. Off the JS thread this deletes a V8 global handle
/// cross-thread, corrupting V8's handle pool (`GlobalHandles`), which
/// surfaces later as `Check failed: node->IsInUse()` aborts or segfaults in
/// unrelated `napi_ref` operations. Until napi-rs routes the deletion
/// through its custom-GC machinery, leak the error instead: one small
/// allocation plus one JS error object kept alive per failed callback.
#[expect(
    clippy::mem_forget,
    reason = "skipping `Drop` is the point: it must not run off the JS thread"
)]
pub(crate) fn reason_and_forget(error: napi::Error) -> String {
    let reason = error.to_string();
    std::mem::forget(error);
    reason
}
