//! The provider's event loop and the messages that drive it.
//!
//! The loop's thread owns the [`ProviderData`] outright; all access goes
//! through [`Message`]s, so no locking is needed. The loop also owns the
//! interval-mining timer, restarting it whenever a request reconfigured it —
//! including to the interval already set, as Hardhat does.

use std::{
    convert::Infallible,
    panic::{self, AssertUnwindSafe},
    sync::Arc,
    time::{Duration, Instant},
};

use crossbeam_channel::{Receiver, Sender};
use edr_chain_spec::TransactionValidation;
use edr_chain_spec_provider::ProviderChainSpec;
use edr_transaction::{IsEip155, IsEip4844, TransactionMut, TransactionType};

use crate::{
    config::IntervalConfig,
    data::ProviderData,
    error::{ProviderError, ProviderErrorForChainSpec},
    mock::SyncCallOverride,
    requests::{self, ProviderRequest},
    spec::SyncProviderSpec,
    time::TimeSinceEpoch,
    ResponseWithCallTraces,
};

/// The response to a [`Message::Request`].
pub(crate) type RequestResponse<ChainSpecT> =
    Result<ResponseWithCallTraces, ProviderErrorForChainSpec<ChainSpecT>>;

/// A completion callback that receives the response to a
/// [`Message::Request`].
///
/// Settles with [`ProviderError::UnexpectedTermination`] if it is dropped
/// without being called, so a caller always receives exactly one response.
pub(crate) struct OnResponse<ChainSpecT: ProviderChainSpec>(
    Option<Box<dyn FnOnce(RequestResponse<ChainSpecT>) + Send>>,
);

impl<ChainSpecT: ProviderChainSpec> OnResponse<ChainSpecT> {
    pub(crate) fn new(callback: Box<dyn FnOnce(RequestResponse<ChainSpecT>) + Send>) -> Self {
        Self(Some(callback))
    }

    /// Invokes the callback with `response`.
    pub(crate) fn call(mut self, response: RequestResponse<ChainSpecT>) {
        let callback = self
            .0
            .take()
            .expect("the callback is taken here or on drop, never both");

        callback(response);
    }
}

impl<ChainSpecT: ProviderChainSpec> Drop for OnResponse<ChainSpecT> {
    fn drop(&mut self) {
        let Some(callback) = self.0.take() else {
            return;
        };

        // The callback may run while a panic unwinds, where a second panic
        // aborts the process.
        let result = panic::catch_unwind(AssertUnwindSafe(move || {
            callback(Err(ProviderError::UnexpectedTermination));
        }));

        if result.is_err() {
            log::error!("A provider response callback panicked while settling a request");
        }
    }
}

/// A message processed by the provider's event loop.
pub(crate) enum Message<ChainSpecT: ProviderChainSpec> {
    /// Handle a single or batched JSON-RPC request, passing the response to
    /// `on_response`.
    Request {
        request: ProviderRequest<ChainSpecT>,
        on_response: OnResponse<ChainSpecT>,
    },
    /// Set (or clear) the call-override callback.
    SetCallOverrideCallback {
        callback: Option<Arc<dyn SyncCallOverride>>,
        ack: Sender<()>,
    },
    /// Set whether traces include the full stack and memory.
    SetVerboseTracing { enabled: bool, ack: Sender<()> },
    /// Log a failed request deserialization through the provider's logger.
    /// Not acknowledged, so a caller on the JS main thread is never blocked.
    LogFailedDeserialization {
        method_name: String,
        error: Box<ProviderErrorForChainSpec<ChainSpecT>>,
    },
}

/// Creates a channel that yields once the next interval-mined block is due.
///
/// Yields nothing if interval mining is disabled. The interval is measured from
/// the end of the previous mine, as Hardhat measures it, so the period between
/// blocks is the interval plus the time spent mining.
///
/// Measuring from the end is what keeps interval mining from starving request
/// handling: the deadline is always a full interval away when the loop next
/// polls, so every cycle leaves a window in which the timer is not ready.
fn next_interval_timer(interval_config: Option<&IntervalConfig>) -> Receiver<Instant> {
    if let Some(config) = interval_config {
        let duration = Duration::from_millis(config.generate_interval().get());
        crossbeam_channel::after(duration)
    } else {
        crossbeam_channel::never()
    }
}

/// Processes messages, taking ownership of `data`, until shutdown.
///
/// Interval mining takes precedence over queued messages. Returns once all
/// message senders are dropped, or `cancellation_receiver` disconnects.
pub(crate) fn run<ChainSpecT, TimerT>(
    mut data: ProviderData<ChainSpecT, TimerT>,
    request_receiver: Receiver<Message<ChainSpecT>>,
    cancellation_receiver: Receiver<Infallible>,
) where
    ChainSpecT: SyncProviderSpec<
        TimerT,
        PooledTransaction: IsEip155,
        SignedTransaction: Default
                               + TransactionMut
                               + TransactionType<Type: IsEip4844>
                               + TransactionValidation<ValidationError: PartialEq>,
    >,
    TimerT: Clone + TimeSinceEpoch,
{
    let mut interval_timer = next_interval_timer(data.interval_config());

    loop {
        crossbeam_channel::select_biased! {
            // Checked first: shutdown must win over queued work.
            recv(cancellation_receiver) -> _ => break,
            // Checked before requests: a due block must not wait behind a long
            // queue. This cannot starve request handling; see
            // `next_interval_timer`.
            recv(interval_timer) -> _ => {
                if let Err(error) = data.interval_mine() {
                    log::error!("Unexpected error while performing interval mining: {error}");
                }
                interval_timer = next_interval_timer(data.interval_config());
            }
            recv(request_receiver) -> message => match message {
                Ok(Message::Request { request, on_response }) => {
                    let response = requests::execute_request(&mut data, request);

                    // Armed before the response is handed back, because
                    // `on_response` serializes it on this thread and that is not
                    // part of the interval.
                    if data.take_interval_reconfigured() {
                        interval_timer = next_interval_timer(data.interval_config());
                    }

                    on_response.call(response);
                }
                Ok(Message::SetCallOverrideCallback { callback, ack }) => {
                    data.set_call_override_callback(callback);

                    // Ignore the error: the caller may have stopped waiting.
                    let _ = ack.send(());
                }
                Ok(Message::SetVerboseTracing { enabled, ack }) => {
                    data.set_verbose_tracing(enabled);

                    // Ignore the error: the caller may have stopped waiting.
                    let _ = ack.send(());
                }
                Ok(Message::LogFailedDeserialization { method_name, error }) => {
                    if let Err(error) = data
                        .logger_mut()
                        .print_method_logs(&method_name, Some(&error))
                    {
                        log::error!("Unexpected error while logging a failed deserialization: {error}");
                    }
                }
                // All senders were dropped; nothing more can arrive.
                Err(_) => break,
            }
        }
    }
}
