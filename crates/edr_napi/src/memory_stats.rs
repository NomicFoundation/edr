#![cfg(feature = "memory-stats")]

//! Types and functions for inspecting memory usage.
//!
//! Exposes the statistics collected by the `mimalloc` allocator that is used
//! as the global allocator of this addon.

use std::{
    ffi::{c_char, c_void, CStr},
    ptr,
};

use napi::bindgen_prelude::BigInt;
use napi_derive::napi;

use crate::cast::TryCast as _;

/// Memory statistics for the process, as reported by `mimalloc`'s
/// `mi_process_info`.
#[napi(object)]
pub struct MemoryStats {
    /// Elapsed wall-clock time of the process, in milliseconds.
    pub elapsed_ms: BigInt,
    /// User time, in milliseconds, as the sum over all threads.
    pub user_ms: BigInt,
    /// System time, in milliseconds.
    pub system_ms: BigInt,
    /// Current resident set size (touched pages), in bytes. This is an OS
    /// process-level measurement. It is precise on Windows and macOS; on
    /// other systems it is estimated using the current committed memory.
    pub current_rss: BigInt,
    /// Peak resident set size, in bytes. This is an OS process-level
    /// measurement.
    pub peak_rss: BigInt,
    /// Current committed memory, in bytes. This is a `mimalloc`-internal
    /// measurement: the amount of read/write accessible memory reserved by
    /// the allocator (precise on Windows, estimated elsewhere).
    pub current_commit: BigInt,
    /// Peak committed memory, in bytes. This is a `mimalloc`-internal
    /// measurement.
    pub peak_commit: BigInt,
    /// Count of hard page faults.
    pub page_faults: BigInt,
}

/// Returns memory statistics for the process, as reported by `mimalloc`'s
/// `mi_process_info`.
#[napi(catch_unwind)]
pub fn memory_stats() -> napi::Result<MemoryStats> {
    let mut elapsed_msecs = 0usize;
    let mut user_msecs = 0usize;
    let mut system_msecs = 0usize;
    let mut current_rss = 0usize;
    let mut peak_rss = 0usize;
    let mut current_commit = 0usize;
    let mut peak_commit = 0usize;
    let mut page_faults = 0usize;

    // SAFETY: All out-parameters point to live `usize` values that outlive
    // the call.
    unsafe {
        libmimalloc_sys::mi_process_info(
            &mut elapsed_msecs,
            &mut user_msecs,
            &mut system_msecs,
            &mut current_rss,
            &mut peak_rss,
            &mut current_commit,
            &mut peak_commit,
            &mut page_faults,
        );
    }

    let elapsed_ms = elapsed_msecs.try_cast()?;
    let user_ms = user_msecs.try_cast()?;
    let system_ms = system_msecs.try_cast()?;
    let current_rss = current_rss.try_cast()?;
    let peak_rss = peak_rss.try_cast()?;
    let current_commit = current_commit.try_cast()?;
    let peak_commit = peak_commit.try_cast()?;
    let page_faults = page_faults.try_cast()?;

    Ok(MemoryStats {
        elapsed_ms,
        user_ms,
        system_ms,
        current_rss,
        peak_rss,
        current_commit,
        peak_commit,
        page_faults,
    })
}

/// Returns a human-readable report of `mimalloc`'s main statistics, i.e. the
/// output of `mi_stats_print_out`.
///
/// The level of detail is fixed at compile time (`MI_STAT`). In EDR's
/// configuration `mimalloc` is compiled with `MI_STAT=0` ("only essential"),
/// regardless of the cargo profile: the report contains arena-level
/// accounting (reserved, committed, purged), page statistics, and process
/// information, but the per-allocation breakdown (block totals and
/// per-size-class bins) is compiled out. Reaching `MI_STAT=2` would require
/// building `libmimalloc-sys` with its `debug` feature, which also enables
/// internal assertions and is expensive.
#[napi(catch_unwind)]
pub fn memory_report() -> String {
    // Pre-allocate so that `append_to_buffer` doesn't reallocate through the
    // global allocator (mimalloc) while `mimalloc` is printing.
    //
    // 2 KiB covers the ~1.3 KiB report that `MI_STAT=0` builds produce.
    //
    // If `MI_STAT > 1`, a variable part is added, which is capped at `MI_BIN_HUGE +
    // 1`; i.e. 74 bin lines. Extrapolating to all 74 bins gives ~7.7 KB.
    let mut buffer = Vec::<u8>::with_capacity(2048);

    // SAFETY: `append_to_buffer` does not unwind and only accesses `arg` as
    // the `Vec<u8>` it points to. `buffer` outlives the call and is not
    // accessed otherwise while `mi_stats_print_out` runs.
    unsafe {
        libmimalloc_sys::mi_stats_print_out(
            Some(append_to_buffer),
            ptr::from_mut(&mut buffer).cast::<c_void>(),
        );
    }

    buffer.shrink_to_fit();

    String::from_utf8_lossy(&buffer).into_owned()
}

/// Output callback for `mi_stats_print_out` that appends the message to the
/// `Vec<u8>` passed through `arg`.
///
/// As it is called from C code, this function must not unwind. It only
/// performs operations that abort rather than unwind on failure: allocation
/// failure in `extend_from_slice` aborts via the global allocation error
/// handler. In addition, the `extern "C"` ABI converts any unexpected panic
/// into an abort instead of undefined behavior.
unsafe extern "C" fn append_to_buffer(msg: *const c_char, arg: *mut c_void) {
    if msg.is_null() || arg.is_null() {
        return;
    }

    // SAFETY: `mimalloc` passes a valid nul-terminated C string that is live
    // for the duration of the callback.
    let message = unsafe { CStr::from_ptr(msg) };
    // SAFETY: `arg` is the pointer to the `Vec<u8>` created in
    // `memory_report`, which is live and not accessed by anything else while
    // `mi_stats_print_out` runs.
    let buffer = unsafe { &mut *arg.cast::<Vec<u8>>() };

    buffer.extend_from_slice(message.to_bytes());
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn memory_stats_reports_process_memory() {
        // Ensure the allocator has something to report.
        let _allocation = vec![0u8; 1024 * 1024];

        let stats = memory_stats().expect("memory statistics should be available");

        let peak_rss: u64 = stats.peak_rss.try_cast().unwrap();
        let current_commit: u64 = stats.current_commit.try_cast().unwrap();
        let peak_commit: u64 = stats.peak_commit.try_cast().unwrap();

        // The peak RSS is an OS process-level measurement, so it is non-zero.
        assert!(peak_rss > 0);

        // Invariant that holds across all platforms and constrains the
        // mapping of `mi_process_info`'s out-parameters to struct fields: the
        // peak commit is the high-water mark of the current commit. Note that
        // `peak_rss >= current_commit` does NOT hold in general: zeroed
        // allocations commit pages without touching them, so the commit can
        // exceed the RSS.
        assert!(peak_commit >= current_commit);
    }

    #[test]
    fn memory_stats_commit_tracks_allocation() {
        fn current_commit() -> u64 {
            memory_stats()
                .expect("memory statistics should be available")
                .current_commit
                .try_cast()
                .unwrap()
        }

        let before = current_commit();

        // Non-zero contents, so that all pages are touched and the allocation
        // drives the RSS in addition to the commit.
        let big = vec![1u8; 256 * 1024 * 1024];
        std::hint::black_box(&big);

        let stats = memory_stats().expect("memory statistics should be available");
        let after: u64 = stats.current_commit.try_cast().unwrap();
        let peak_rss: u64 = stats.peak_rss.try_cast().unwrap();

        // The live 256 MiB allocation must show up in the committed memory,
        // with slack for concurrent deallocations by other tests.
        assert!(after >= before + 200 * 1024 * 1024);
        // The touched 256 MiB must have driven the OS peak RSS at least this
        // high, pinning `peak_rss` to an RSS out-parameter.
        assert!(peak_rss >= 200 * 1024 * 1024);
    }

    #[test]
    fn memory_report_is_non_empty() {
        let report = memory_report();

        assert!(!report.is_empty());
    }
}
