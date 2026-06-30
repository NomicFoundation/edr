use std::{marker::PhantomData, sync::Arc};

use crossbeam_channel::{bounded, unbounded, Sender};
use edr_chain_spec::{HardforkChainSpec, TransactionValidation};
use edr_solidity::contract_decoder::ContractDecoder;
use edr_transaction::{IsEip155, IsEip4844, TransactionMut, TransactionType};
use edr_utils_sync::CancellableThread;
use parking_lot::RwLock;
use tokio::runtime;

use crate::{
    backend::{self, BackendRequest},
    config::ProviderConfig,
    data::ProviderData,
    error::{CreationErrorForChainSpec, ProviderErrorForChainSpec},
    logger::SyncLogger,
    mock::SyncCallOverride,
    requests::ProviderRequest,
    spec::{ProviderSpec, SyncProviderSpec},
    time::{CurrentTime, TimeSinceEpoch},
    ResponseWithCallTraces, SyncSubscriberCallback,
};

/// A JSON-RPC provider for Ethereum.
///
/// The provider owns a dedicated background thread that holds the
/// [`ProviderData`] and processes requests one at a time. Requests are sent
/// over a channel and queue up in order; interval mining (if enabled) is driven
/// by the same thread and takes precedence whenever a block is due. The thread
/// is shut down and joined when the `Provider` is dropped.
///
/// This type can be shared (e.g. behind an `Arc`) and called from multiple
/// threads concurrently; each call queues its request and blocks until the
/// background thread replies.
pub struct Provider<ChainSpecT: ProviderSpec<TimerT>, TimerT: Clone + TimeSinceEpoch = CurrentTime>
{
    request_sender: Sender<BackendRequest<ChainSpecT>>,
    _thread: CancellableThread,
    _phantom: PhantomData<fn() -> TimerT>,
}

impl<ChainSpecT: SyncProviderSpec<TimerT>, TimerT: Clone + TimeSinceEpoch>
    Provider<ChainSpecT, TimerT>
{
    /// Sends a request to the background thread and blocks until it replies.
    ///
    /// `new_request_fn` receives the sending end of a freshly created reply
    /// channel and must embed it in the [`BackendRequest`] it returns; the
    /// background thread sends the `ResponseT` back on that channel once it
    /// has processed the request.
    fn send_request<ResponseT>(
        &self,
        new_request_fn: impl FnOnce(Sender<ResponseT>) -> BackendRequest<ChainSpecT>,
    ) -> ResponseT {
        const BACKEND_THREAD_TERMINATED: &str = "the provider background thread has terminated";

        let (response_sender, response_receiver) = bounded(1);
        self.request_sender
            .send(new_request_fn(response_sender))
            .expect(BACKEND_THREAD_TERMINATED);

        response_receiver.recv().expect(BACKEND_THREAD_TERMINATED)
    }

    /// Blocking method to log a failed deserialization.
    pub fn log_failed_deserialization(
        &self,
        method_name: &str,
        error: ProviderErrorForChainSpec<ChainSpecT>,
    ) -> Result<(), ProviderErrorForChainSpec<ChainSpecT>> {
        self.send_request(|ack| BackendRequest::LogFailedDeserialization {
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
    /// This spawns the dedicated background thread that owns the provider's
    /// state. Construction of the [`ProviderData`] happens on that thread and
    /// any error is reported back before this method returns.
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
                backend::run(data, request_receiver, cancellation_receiver);
            })
            .expect("failed to spawn the provider background thread");

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
        self.send_request(|ack| BackendRequest::SetCallOverrideCallback {
            callback: call_override_callback,
            ack,
        });
    }

    pub fn set_verbose_tracing(&self, enabled: bool) {
        self.send_request(|ack| BackendRequest::SetVerboseTracing { enabled, ack });
    }

    /// Blocking method to handle a request.
    ///
    /// The request is queued on the background thread and this method blocks
    /// until the response is available.
    pub fn handle_request(
        &self,
        request: ProviderRequest<ChainSpecT>,
    ) -> Result<ResponseWithCallTraces, ProviderErrorForChainSpec<ChainSpecT>> {
        self.send_request(|response_sender| BackendRequest::Request {
            request,
            response_sender,
        })
    }
}
