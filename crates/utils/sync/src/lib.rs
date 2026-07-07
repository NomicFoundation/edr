//! Synchronization utilities used across the EDR codebase.

#![warn(missing_docs)]

mod cancellable_thread;

pub use self::cancellable_thread::CancellableThread;
