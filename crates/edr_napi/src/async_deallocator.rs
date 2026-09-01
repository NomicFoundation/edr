use std::{io, sync::Arc};

use crossbeam_channel::{select_biased, unbounded, SendError, Sender};
use derive_where::derive_where;
use edr_napi_core::provider::SyncProvider;
use edr_utils_sync::{CancellableThread, MAX_THREAD_NAME_LEN};
use napi::tokio::runtime;

/// Name of the thread that drops providers.
pub const PROVIDER_THREAD_NAME: &str = "edr-drop-prov";

/// Name of the thread that drops responses.
pub const RESPONSE_THREAD_NAME: &str = "edr-drop-resp";

const _: () = {
    assert!(PROVIDER_THREAD_NAME.len() <= MAX_THREAD_NAME_LEN);
    assert!(RESPONSE_THREAD_NAME.len() <= MAX_THREAD_NAME_LEN);
};

/// Senders for off-loading dropped values to their deallocator threads.
///
/// Dropping the last handle to a provider joins that provider's event-loop
/// thread. Providers therefore need a deallocator thread separate from the
/// responses'.
pub struct Deallocators {
    /// Accepts dropped providers.
    pub provider: AsyncDeallocatorSender<Arc<dyn SyncProvider>>,
    /// Accepts dropped responses.
    pub response: AsyncDeallocatorSender<edr_napi_core::spec::Response>,
}

/// Owns a dedicated OS thread that drops values of type `T` outside of the
/// calling thread. Producers obtain cloneable senders via [`Self::sender`] and
/// call [`AsyncDeallocatorSender::deallocate`].
///
/// If the dedicated thread is no longer accepting work (e.g. because the
/// owning [`AsyncDeallocator`] has been dropped, or the thread has panicked),
/// `deallocate` falls back to dropping the value on the tokio blocking pool.
pub struct AsyncDeallocator<T: Send + 'static> {
    sender: Sender<T>,
    runtime: runtime::Handle,
    // Dropping this disconnects the cancellation channel and joins the
    // dedicated thread, so no explicit shutdown is needed.
    _thread: CancellableThread,
}

impl<T: Send + 'static> AsyncDeallocator<T> {
    /// Constructs a new instance.
    ///
    /// `thread_name` must be at most [`MAX_THREAD_NAME_LEN`] bytes long, or the
    /// OS receives it truncated.
    pub fn new(thread_name: String, runtime: runtime::Handle) -> io::Result<Self> {
        let (sender, receiver) = unbounded::<T>();

        let thread = CancellableThread::spawn(thread_name, move |cancellation_receiver| {
            loop {
                // `select_biased!` picks the first listed branch when multiple
                // arms are ready, so cancellation always wins over pending work.
                select_biased! {
                    // Cancellation channel was disconnected by dropping the CancellableThread.
                    recv(cancellation_receiver) -> _ => break,
                    recv(receiver) -> msg => match msg {
                        Ok(value) => drop(value),
                        // All senders dropped; no more work can arrive.
                        Err(_) => break,
                    },
                }
            }
        })?;

        Ok(Self {
            sender,
            runtime,
            _thread: thread,
        })
    }

    /// Returns a cloneable handle for enqueueing values to be dropped on the
    /// dedicated thread.
    pub fn sender(&self) -> AsyncDeallocatorSender<T> {
        AsyncDeallocatorSender {
            sender: self.sender.clone(),
            runtime: self.runtime.clone(),
        }
    }
}

/// A cheap, cloneable handle for sending values to an [`AsyncDeallocator`].
///
/// [`Self::deallocate`] is infallible from the caller's perspective: if the
/// dedicated thread is unavailable, it falls back to the tokio blocking pool.
#[derive_where(Clone)]
pub struct AsyncDeallocatorSender<T: Send + 'static> {
    sender: Sender<T>,
    runtime: runtime::Handle,
}

impl<T: Send + 'static> AsyncDeallocatorSender<T> {
    /// Enqueues `value` for asynchronous deallocation on the dedicated
    /// dropper thread, falling back to the tokio blocking pool if that
    /// thread is unavailable.
    #[inline]
    pub fn deallocate(&self, value: T) {
        if let Err(SendError(value)) = self.sender.send(value) {
            fallback(&self.runtime, value);
        }
    }
}

/// The dedicated thread is unavailable — fall back to dropping `value` on
/// the tokio blocking pool. Marked `#[cold]` and `#[inline(never)]` so the
/// common `Ok` path in [`AsyncDeallocatorSender::deallocate`] stays
/// branch-free after inlining.
///
/// The tokio blocking pool can shrink idle threads, so a fallback drop can
/// pay the cost of spawning a fresh OS thread; that's why this is a
/// fallback rather than the primary path.
#[cold]
#[inline(never)]
fn fallback<T: Send + 'static>(runtime: &runtime::Handle, value: T) {
    runtime.spawn_blocking(move || {
        drop(value);
    });
}
