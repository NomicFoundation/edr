use std::{
    convert::Infallible,
    sync::Arc,
    time::{Duration, Instant},
};

use crossbeam_channel::{Receiver, Sender};
use edr_chain_spec::TransactionValidation;
use edr_chain_spec_provider::ProviderChainSpec;
use edr_transaction::{IsEip155, IsEip4844, TransactionMut, TransactionType};

use crate::{
    data::ProviderData,
    error::{ProviderError, ProviderErrorForChainSpec},
    mock::SyncCallOverride,
    requests::{self, ProviderRequest},
    spec::SyncProviderSpec,
    time::TimeSinceEpoch,
    ResponseWithCallTraces,
};

/// The response to a [`BackendRequest::Request`].
pub(crate) type RequestResponse<ChainSpecT> =
    Result<ResponseWithCallTraces, ProviderErrorForChainSpec<ChainSpecT>>;

/// A completion callback that receives the response to a
/// [`BackendRequest::Request`].
pub(crate) type OnResponse<ChainSpecT> = Box<dyn FnOnce(RequestResponse<ChainSpecT>) + Send>;

/// A message processed by the provider's background thread.
///
/// The thread owns the [`ProviderData`] outright; all access goes through these
/// messages so that requests and interval mining are serialized on a single
/// thread without any locking.
pub(crate) enum BackendRequest<ChainSpecT: ProviderChainSpec> {
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
    /// Toggle whether traces include the full stack and memory.
    SetVerboseTracing { enabled: bool, ack: Sender<()> },
    /// Log a failed request deserialization through the provider's logger.
    LogFailedDeserialization {
        method_name: String,
        error: Box<ProviderErrorForChainSpec<ChainSpecT>>,
        ack: Sender<Result<(), ProviderErrorForChainSpec<ChainSpecT>>>,
    },
}

/// Creates a channel that yields a message whenever the next interval-mined
/// block is due, if interval mining is enabled. Otherwise, creates a channel
/// that never yields.
fn next_interval_timer(
    interval_config: Option<&crate::config::IntervalConfig>,
) -> Receiver<Instant> {
    if let Some(config) = interval_config {
        let duration = Duration::from_millis(config.generate_interval());
        crossbeam_channel::after(duration)
    } else {
        crossbeam_channel::never()
    }
}

/// The event loop run by the provider's dedicated background thread.
///
/// It processes incoming requests in order while giving interval mining
/// precedence whenever a block is due. The loop owns `data` and runs until the
/// `cancellation_receiver` is disconnected (by [`crate::Provider`]'s `Drop`
/// dropping the matching sender), or all request senders are dropped.
pub(super) fn run<ChainSpecT, TimerT>(
    mut data: ProviderData<ChainSpecT, TimerT>,
    request_receiver: Receiver<BackendRequest<ChainSpecT>>,
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
            // Highest priority. The cancellation channel carries `Infallible`, so
            // the only event it can ever yield is disconnection, signalled by
            // `Provider::drop` (which runs off the JS thread via the N-API
            // AsyncDeallocator).
            recv(cancellation_receiver) -> _ => break,
            // Interval mining takes precedence over incoming requests. An overdue
            // deadline yields a zero duration, so `after` is immediately ready.
            recv(interval_timer) -> _ => {
                if let Err(error) = data.interval_mine() {
                    log::error!("Unexpected error while performing interval mining: {error}");
                }
                interval_timer = next_interval_timer(data.interval_config());
            }
            recv(request_receiver) -> message => match message {
                Ok(BackendRequest::Request { request, on_response }) => {
                    let current_interval = data.interval_config().cloned();

                    let response = requests::execute_request(&mut data, request);

                    on_response(response);

                    // `evm_setIntervalMining` may have changed the configuration.
                    if data.interval_config() != current_interval.as_ref() {
                        interval_timer = next_interval_timer(data.interval_config());
                    }
                }
                Ok(BackendRequest::SetCallOverrideCallback { callback, ack }) => {
                    data.set_call_override_callback(callback);

                    // Ignore the error: the caller may have stopped waiting.
                    let _ = ack.send(());
                }
                Ok(BackendRequest::SetVerboseTracing { enabled, ack }) => {
                    data.set_verbose_tracing(enabled);

                    // Ignore the error: the caller may have stopped waiting.
                    let _ = ack.send(());
                }
                Ok(BackendRequest::LogFailedDeserialization { method_name, error, ack }) => {
                    let result = data
                        .logger_mut()
                        .print_method_logs(&method_name, Some(&error))
                        .map_err(ProviderError::Logger);

                    // Ignore the error: the caller may have stopped waiting.
                    let _ = ack.send(result);
                }
                // All request senders were dropped — backstop in case the
                // shutdown signal is not used.
                Err(_) => break,
            }
        }
    }

    // Settle any requests that were still queued when the loop exited;
    // otherwise their callers would never receive a response (e.g. a pending
    // JS promise would never settle). Ack-style messages are simply dropped:
    // disconnecting their reply channel unblocks the caller.
    while let Ok(message) = request_receiver.try_recv() {
        if let BackendRequest::Request { on_response, .. } = message {
            on_response(Err(ProviderError::UnexpectedTermination));
        }
    }
}
