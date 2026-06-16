use std::{convert::Infallible, io, thread::JoinHandle};

use crossbeam_channel::{bounded, Receiver, Sender};

/// Owns a dedicated OS thread together with a cancellation channel used to
/// shut it down. The closure passed to [`Self::spawn`] receives the receiving
/// end of that channel and should treat its disconnection as the signal to
/// return.
///
/// # Shutdown via channel disconnection
///
/// On [`Self::cancel_and_join`], the thread is signalled to exit by
/// **disconnecting** the cancellation channel — that is, by dropping its
/// `Sender`, rather than by sending a sentinel message. The channel's item
/// type is [`std::convert::Infallible`], so the compiler statically guarantees
/// that no message can ever be sent on it; thus guaranteeing that
/// disconnection is the only event that can ever occur on it.
///
/// This idiom is described in the async-std book's
/// [Handling Disconnection](https://book.async.rs/tutorial/handling_disconnection.html)
/// chapter: "Closing a channel is a synchronization event, so we don't need
/// to send a shutdown message, we can just drop the sender. This way, we
/// statically guarantee that we issue shutdown exactly once, even if we
/// early return via `?` or panic."
pub struct CancellableThread {
    cancellation_sender: Sender<Infallible>,
    thread: JoinHandle<()>,
}

impl CancellableThread {
    /// Spawns `f` on a dedicated OS thread, handing it the receiving end of a
    /// cancellation channel.
    ///
    /// See the "Shutdown via channel disconnection" section of
    /// [`CancellableThread`]'s docs for why this channel is parameterized
    /// with `Infallible` and signalled by sender-drop.
    pub fn spawn<F>(name: String, f: F) -> io::Result<Self>
    where
        F: FnOnce(Receiver<Infallible>) + Send + 'static,
    {
        let (cancellation_sender, cancellation_receiver) = bounded::<Infallible>(1);

        let thread = std::thread::Builder::new()
            .name(name)
            .spawn(move || f(cancellation_receiver))?;

        Ok(Self {
            cancellation_sender,
            thread,
        })
    }

    /// Signals the thread to exit by disconnecting the cancellation channel,
    /// then waits for it to finish.
    pub fn cancel_and_join(self) {
        // Drop the cancellation sender, disconnecting the cancellation
        // channel and waking the thread.
        drop(self.cancellation_sender);

        let thread_name = self.thread.thread().name().unwrap_or("unnamed").to_owned();
        if let Err(error) = self.thread.join() {
            tracing::error!("'{thread_name}' thread panicked: {error:?}");
        }
    }
}
