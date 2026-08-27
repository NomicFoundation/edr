use std::{marker::PhantomData, sync::Arc};

use crossbeam_channel::{bounded, unbounded, Sender};
use edr_chain_spec::{HardforkChainSpec, TransactionValidation};
use edr_solidity::contract_decoder::ContractDecoder;
use edr_transaction::{IsEip155, IsEip4844, TransactionMut, TransactionType};
use edr_utils_sync::CancellableThread;
use parking_lot::RwLock;
use tokio::runtime;

use crate::{
    config::ProviderConfig,
    data::ProviderData,
    error::{CreationErrorForChainSpec, ProviderError, ProviderErrorForChainSpec},
    event_loop::{self, Message},
    logger::SyncLogger,
    mock::SyncCallOverride,
    requests::ProviderRequest,
    spec::{ProviderSpec, SyncProviderSpec},
    time::{CurrentTime, TimeSinceEpoch},
    ResponseWithCallTraces, SyncSubscriberCallback,
};

const EVENT_LOOP_TERMINATED: &str = "the provider's event loop has terminated";

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
    /// Sends a message to the event loop and blocks until it replies.
    ///
    /// `new_request_fn` must embed the provided reply sender in the [`Message`]
    /// it returns.
    fn send_request_and_wait<ResponseT>(
        &self,
        new_request_fn: impl FnOnce(Sender<ResponseT>) -> Message<ChainSpecT>,
    ) -> ResponseT {
        let (response_sender, response_receiver) = bounded(1);
        self.request_sender
            .send(new_request_fn(response_sender))
            .expect(EVENT_LOOP_TERMINATED);

        response_receiver.recv().expect(EVENT_LOOP_TERMINATED)
    }

    /// Blocking method to log a failed deserialization.
    pub fn log_failed_deserialization(
        &self,
        method_name: &str,
        error: ProviderErrorForChainSpec<ChainSpecT>,
    ) -> Result<(), ProviderErrorForChainSpec<ChainSpecT>> {
        self.send_request_and_wait(|ack| Message::LogFailedDeserialization {
            method_name: method_name.to_string(),
            error: Box::new(error),
            ack,
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
        config: ProviderConfig<<ChainSpecT as HardforkChainSpec>::Hardfork>,
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
            CancellableThread::spawn("edr-provider".to_owned(), move |cancellation_receiver| {
                event_loop::run(data, request_receiver, cancellation_receiver);
            })
            .expect("failed to spawn the provider thread");

        Ok(Self {
            request_sender,
            _thread: thread,
            _phantom: PhantomData,
        })
    }

    /// Set to `true` to make the traces returned with `eth_call`,
    /// `eth_estimateGas`, `eth_sendRawTransaction`, `eth_sendTransaction`,
    /// `evm_mine`, `hardhat_mine` include the full stack and memory. Set to
    /// `false` to disable this.
    pub fn set_call_override_callback(
        &self,
        call_override_callback: Option<Arc<dyn SyncCallOverride>>,
    ) {
        self.send_request_and_wait(|ack| Message::SetCallOverrideCallback {
            callback: call_override_callback,
            ack,
        });
    }

    pub fn set_verbose_tracing(&self, enabled: bool) {
        self.send_request_and_wait(|ack| Message::SetVerboseTracing { enabled, ack });
    }

    /// Blocking method to handle a request.
    ///
    /// Enqueues the request with [`Self::enqueue_request`] and blocks until the
    /// response is available.
    pub fn handle_request(
        &self,
        request: ProviderRequest<ChainSpecT>,
    ) -> Result<ResponseWithCallTraces, ProviderErrorForChainSpec<ChainSpecT>> {
        let (response_sender, response_receiver) = bounded(1);

        self.enqueue_request(
            request,
            Box::new(move |response| {
                // Ignore the error: the caller may have stopped waiting.
                let _ = response_sender.send(response);
            }),
        );

        response_receiver.recv().expect(EVENT_LOOP_TERMINATED)
    }

    /// Enqueues a request, invoking `on_response` from the provider's thread
    /// once the response is available.
    ///
    /// This method never executes the request on the calling thread and
    /// returns immediately, without waiting for the request to be handled.
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
        if let Err(error) = self.request_sender.send(Message::Request {
            request,
            on_response,
        }) {
            let Message::Request { on_response, .. } = error.0 else {
                unreachable!("the returned message is the one that failed to send")
            };

            on_response(Err(ProviderError::UnexpectedTermination));
        }
    }
}
