use std::{marker::PhantomData, sync::Arc};

use crossbeam_channel::{bounded, unbounded, RecvError, Sender};
use edr_chain_spec::{ProtocolHardforkChainSpec, TransactionValidation};
use edr_solidity::contract_decoder::ContractDecoder;
use edr_transaction::{IsEip155, IsEip4844, TransactionMut, TransactionType};
use edr_utils_sync::{CancellableThread, MAX_THREAD_NAME_LEN};
use parking_lot::RwLock;
use tokio::runtime;

use crate::{
    config::ProviderConfig,
    data::ProviderData,
    error::{CreationErrorForChainSpec, ProviderError, ProviderErrorForChainSpec},
    event_loop::{self, Message, OnResponse},
    logger::SyncLogger,
    mock::SyncCallOverride,
    requests::ProviderRequest,
    spec::{ProviderSpec, SyncProviderSpec},
    time::{CurrentTime, TimeSinceEpoch},
    ResponseWithCallTraces, SyncSubscriberCallback,
};

/// Name of the thread that owns the [`ProviderData`] and serves requests.
const THREAD_NAME: &str = "edr-provider";

const _: () = assert!(THREAD_NAME.len() <= MAX_THREAD_NAME_LEN);

/// A JSON-RPC provider for Ethereum.
///
/// Requests are queued on a dedicated thread that owns the [`ProviderData`]
/// and handles them one at a time. Interval mining, if enabled, runs on that
/// same thread and takes precedence over queued requests.
///
/// The thread is shut down and joined when the provider is dropped.
///
/// This type can be shared (e.g. behind an `Arc`) and called from multiple
/// threads concurrently. Requests are handled in the order they are queued.
pub struct Provider<ChainSpecT: ProviderSpec<TimerT>, TimerT: Clone + TimeSinceEpoch = CurrentTime>
{
    request_sender: Sender<Message<ChainSpecT>>,
    _thread: CancellableThread,
    _phantom: PhantomData<fn() -> TimerT>,
}

impl<ChainSpecT: SyncProviderSpec<TimerT>, TimerT: Clone + TimeSinceEpoch>
    Provider<ChainSpecT, TimerT>
{
    /// Creates a reply channel, lets `enqueue_fn` dispatch a message embedding
    /// its sender, and blocks until the reply arrives.
    ///
    /// A disconnected reply channel means the event loop terminated without
    /// replying.
    fn wait_for_reply<ResponseT>(
        enqueue_fn: impl FnOnce(Sender<ResponseT>),
    ) -> Result<ResponseT, ProviderErrorForChainSpec<ChainSpecT>> {
        let (response_sender, response_receiver) = bounded(1);

        enqueue_fn(response_sender);

        response_receiver
            .recv()
            .map_err(|RecvError| ProviderError::UnexpectedTermination)
    }

    /// Sends a message to the event loop, handing it back if the event loop has
    /// terminated.
    fn send_message(&self, message: Message<ChainSpecT>) -> Result<(), Message<ChainSpecT>> {
        self.request_sender.send(message).map_err(|error| error.0)
    }

    /// Queues a failed deserialization to be logged.
    ///
    /// Does not wait for the log to be printed, so that a caller on the JS main
    /// thread is not blocked behind the queued requests.
    pub fn log_failed_deserialization(
        &self,
        method_name: &str,
        error: ProviderErrorForChainSpec<ChainSpecT>,
    ) {
        // A terminated event loop cannot log; there is nothing to report to.
        let _ = self.send_message(Message::LogFailedDeserialization {
            method_name: method_name.to_string(),
            error: Box::new(error),
        });
    }

    /// Sets the call override callback, or clears it when passed `None`.
    pub fn set_call_override_callback(
        &self,
        call_override_callback: Option<Arc<dyn SyncCallOverride>>,
    ) -> Result<(), ProviderErrorForChainSpec<ChainSpecT>> {
        Self::wait_for_reply(|ack| {
            let _ = self.send_message(Message::SetCallOverrideCallback {
                callback: call_override_callback,
                ack,
            });
        })
    }

    /// Set to `true` to make the traces returned with `eth_call`,
    /// `eth_estimateGas`, `eth_sendRawTransaction`, `eth_sendTransaction`,
    /// `evm_mine`, `hardhat_mine` include the full stack and memory. Set to
    /// `false` to disable this.
    pub fn set_verbose_tracing(
        &self,
        enabled: bool,
    ) -> Result<(), ProviderErrorForChainSpec<ChainSpecT>> {
        Self::wait_for_reply(|ack| {
            let _ = self.send_message(Message::SetVerboseTracing { enabled, ack });
        })
    }
}

impl<
        ChainSpecT: SyncProviderSpec<
            TimerT,
            PooledTransaction: IsEip155,
            SignedTransaction: Default
                                   + TransactionMut
                                   + TransactionType<Type: IsEip4844>
                                   + TransactionValidation<ValidationError: PartialEq>,
        >,
        TimerT: Clone + TimeSinceEpoch,
    > Provider<ChainSpecT, TimerT>
{
    /// Constructs a new instance.
    ///
    /// Spawns the dedicated thread that takes ownership of the provider's
    /// state.
    pub fn new(
        runtime: runtime::Handle,
        logger: Box<dyn SyncLogger<ChainSpecT, TimerT>>,
        subscriber_callback: Box<
            dyn SyncSubscriberCallback<ChainSpecT::Block, ChainSpecT::SignedTransaction>,
        >,
        config: ProviderConfig<<ChainSpecT as ProtocolHardforkChainSpec>::ProtocolHardfork>,
        contract_decoder: Arc<RwLock<ContractDecoder>>,
        timer: TimerT,
    ) -> Result<Self, CreationErrorForChainSpec<ChainSpecT>> {
        let data = ProviderData::new(
            runtime,
            logger,
            subscriber_callback,
            config,
            contract_decoder,
            timer,
        )?;

        let (request_sender, request_receiver) = unbounded();

        let thread =
            CancellableThread::spawn(THREAD_NAME.to_owned(), move |cancellation_receiver| {
                event_loop::run(data, request_receiver, cancellation_receiver);
            })
            .expect("failed to spawn the provider thread");

        Ok(Self {
            request_sender,
            _thread: thread,
            _phantom: PhantomData,
        })
    }

    /// Blocking method to handle a request.
    ///
    /// Enqueues the request with [`Self::enqueue_request`] and blocks until the
    /// response is available.
    pub fn handle_request(
        &self,
        request: ProviderRequest<ChainSpecT>,
    ) -> Result<ResponseWithCallTraces, ProviderErrorForChainSpec<ChainSpecT>> {
        Self::wait_for_reply(|response_sender| {
            self.enqueue_request(
                request,
                Box::new(move |response| {
                    // Ignore the error: the caller may have stopped waiting.
                    let _ = response_sender.send(response);
                }),
            );
        })?
    }

    /// Enqueues a request, invoking `on_response` from the provider's thread
    /// once the response is available.
    ///
    /// Does not wait for the request to be handled.
    ///
    /// If the provider's thread has terminated, `on_response` is invoked on the
    /// calling thread with [`ProviderError::UnexpectedTermination`].
    pub fn enqueue_request(
        &self,
        request: ProviderRequest<ChainSpecT>,
        on_response: Box<
            dyn FnOnce(Result<ResponseWithCallTraces, ProviderErrorForChainSpec<ChainSpecT>>)
                + Send,
        >,
    ) {
        // Dropping the message settles `on_response` with
        // `UnexpectedTermination`.
        let _ = self.send_message(Message::Request {
            request,
            on_response: OnResponse::new(on_response),
        });
    }
}
