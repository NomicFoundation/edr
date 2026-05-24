use std::{convert::Infallible, thread::JoinHandle};

use crossbeam_channel::{bounded, select_biased, unbounded, SendError, Sender};
use napi::tokio::runtime;

/// Owns a dedicated OS thread that drops values of type `T` outside of the
/// calling thread. Producers obtain cloneable senders via [`Self::sender`] and
/// call [`AsyncDeallocatorSender::deallocate`].
///
/// If the dedicated thread is no longer accepting work (e.g. because the
/// owning [`AsyncDeallocator`] has been dropped, or the thread has panicked),
/// `deallocate` falls back to dropping the value on the tokio blocking pool.
///
/// # Shutdown via channel disconnection
///
/// On `Drop`, the dedicated thread is signalled to exit by **disconnecting**
/// a dedicated cancellation channel — that is, by dropping its `Sender`,
/// rather than by sending a sentinel message. The channel's item type is
/// [`std::convert::Infallible`], so the compiler statically guarantees that no
/// message can ever be sent on it; thus guaranteeing that disconnection is the
/// only event that can ever occur on it.
///
/// This idiom is described in the async-std book's
/// [Handling Disconnection](https://book.async.rs/tutorial/handling_disconnection.html)
/// chapter: "Closing a channel is a synchronization event, so we don't need
/// to send a shutdown message, we can just drop the sender. This way, we
/// statically guarantee that we issue shutdown exactly once, even if we
/// early return via `?` or panic."
pub struct AsyncDeallocator<T: Send + 'static> {
    sender: Sender<T>,
    runtime: runtime::Handle,
    cancellation_sender: Option<Sender<Infallible>>,
    thread: Option<JoinHandle<()>>,
}

impl<T: Send + 'static> AsyncDeallocator<T> {
    /// Constructs a new instance.
    pub fn new(runtime: runtime::Handle) -> Self {
        let (sender, receiver) = unbounded::<T>();
        // See the "Shutdown via channel disconnection" section of
        // [`AsyncDeallocator`]'s docs for why this channel is parameterized
        // with `Infallible` and signalled by sender-drop.
        let (cancellation_sender, cancellation_receiver) = bounded::<Infallible>(1);

        let thread = std::thread::spawn(move || loop {
            // `select_biased!` picks the first listed branch when multiple
            // arms are ready, so cancellation always wins over pending work.
            select_biased! {
                // Cancellation channel was disconnected by AsyncDeallocator::drop.
                recv(cancellation_receiver) -> _ => break,
                recv(receiver) -> msg => match msg {
                    Ok(value) => drop(value),
                    // All senders dropped; no more work can arrive.
                    Err(_) => break,
                },
            }
        });

        Self {
            sender,
            runtime,
            cancellation_sender: Some(cancellation_sender),
            thread: Some(thread),
        }
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

impl<T: Send + 'static> Drop for AsyncDeallocator<T> {
    fn drop(&mut self) {
        // Drop the cancellation sender, disconnecting the cancellation
        // channel and waking the dropper thread.
        self.cancellation_sender.take();

        if let Some(handle) = self.thread.take()
            && let Err(error) = handle.join()
        {
            tracing::error!("AsyncDeallocator thread panicked: {error:?}");
        }
    }
}

/// A cheap, cloneable handle for sending values to an [`AsyncDeallocator`].
///
/// [`Self::deallocate`] is infallible from the caller's perspective: if the
/// dedicated thread is unavailable, it falls back to the tokio blocking pool.
pub struct AsyncDeallocatorSender<T: Send + 'static> {
    sender: Sender<T>,
    runtime: runtime::Handle,
}

impl<T: Send + 'static> Clone for AsyncDeallocatorSender<T> {
    fn clone(&self) -> Self {
        Self {
            sender: self.sender.clone(),
            runtime: self.runtime.clone(),
        }
    }
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
