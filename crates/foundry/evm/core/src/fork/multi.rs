//! Support for running multiple fork backends
//!
//! The design is similar to the single `SharedBackend`, `BackendHandler` but
//! supports multiple concurrently active pairs at once.

use std::{
    collections::HashMap,
    fmt::{self, Write},
    pin::Pin,
    sync::{
        atomic::AtomicUsize,
        mpsc::{channel as oneshot_channel, Sender as OneshotSender},
        Arc,
    },
    time::Duration,
};

use alloy_consensus::BlockHeader;
use alloy_network::BlockResponse;
use foundry_fork_db::{cache::BlockchainDbMeta, BackendHandler, BlockchainDb, SharedBackend};
use futures::{
    channel::mpsc::{channel, Receiver, Sender},
    stream::{Fuse, Stream},
    task::{Context, Poll},
    Future, FutureExt, StreamExt,
};

use super::CreateFork;
use crate::{
    evm_context::{BlockEnvTr, EvmEnv, HardforkTr, TransactionEnvTr},
    fork::provider::{ProviderBuilder, RetryProvider},
};

/// The _unique_ identifier for a specific fork, this could be the name of the
/// network a custom descriptive name.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct ForkId(pub String);

impl ForkId {
    /// Returns the identifier for a Fork from a URL and block number.
    pub fn new(url: &str, num: Option<u64>) -> Self {
        let mut id = url.to_string();
        id.push('@');
        match num {
            Some(n) => write!(id, "{n:#x}").unwrap(),
            None => id.push_str("latest"),
        }
        ForkId(id)
    }

    /// Returns the identifier of the fork.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for ForkId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl<T: Into<String>> From<T> for ForkId {
    fn from(id: T) -> Self {
        Self(id.into())
    }
}

/// The Sender half of multi fork pair.
/// Can send requests to the `MultiForkHandler` to create forks
#[derive(Clone, Debug)]
pub struct MultiFork<BlockT, TxT, HardforkT> {
    /// Channel to send `Request`s to the handler
    handler: Sender<Request<BlockT, TxT, HardforkT>>,
    /// Ensures that all rpc resources get flushed properly
    _shutdown: Arc<ShutDownMultiFork<BlockT, TxT, HardforkT>>,
}

// === impl MultiForkBackend ===

impl<BlockT: BlockEnvTr, TxT: TransactionEnvTr, HardforkT: HardforkTr>
    MultiFork<BlockT, TxT, HardforkT>
{
    /// Creates a new pair multi fork pair
    pub fn new() -> (Self, MultiForkHandler<BlockT, TxT, HardforkT>) {
        let (handler, handler_rx) = channel(1);
        let _shutdown = Arc::new(ShutDownMultiFork {
            handler: Some(handler.clone()),
        });
        (
            Self { handler, _shutdown },
            MultiForkHandler::new(handler_rx),
        )
    }

    /// Creates a new pair and spawns the `MultiForkHandler` on a background
    /// thread.
    pub fn spawn() -> Self {
        trace!(target: "fork::multi", "spawning multifork");

        let (fork, mut handler) = Self::new();
        // spawn a light-weight thread with a thread-local async runtime just for
        // sending and receiving data from the remote client(s)
        std::thread::Builder::new()
            .name("multi-fork-backend".into())
            .spawn(move || {
                let rt = tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                    .expect("failed to build tokio runtime");

                rt.block_on(async move {
                    // flush cache every 60s, this ensures that long-running fork tests get their
                    // cache flushed from time to time
                    // NOTE: we install the interval here because the `tokio::timer::Interval`
                    // requires a rt
                    handler.set_flush_cache_interval(Duration::from_secs(60));
                    handler.await;
                });
            })
            .expect("failed to spawn thread");
        trace!(target: "fork::multi", "spawned MultiForkHandler thread");
        fork
    }

    /// Returns a fork backend
    ///
    /// If no matching fork backend exists it will be created
    pub fn create_fork(
        &self,
        fork: CreateFork<BlockT, TxT, HardforkT>,
    ) -> eyre::Result<(ForkId, SharedBackend, EvmEnv<BlockT, TxT, HardforkT>)> {
        trace!(
            "Creating new fork, url={}, block={:?}",
            fork.url,
            fork.evm_opts.fork_block_number
        );
        let (sender, rx) = oneshot_channel();
        let req = Request::CreateFork(Box::new(fork), sender);
        self.handler
            .clone()
            .try_send(req)
            .map_err(|e| eyre::eyre!("{:?}", e))?;
        let (fork_id, shared_backend, env) = rx.recv()??;
        Ok((fork_id, shared_backend, env))
    }

    /// Rolls the block of the fork
    ///
    /// If no matching fork backend exists it will be created
    pub fn roll_fork(
        &self,
        fork: ForkId,
        block: u64,
    ) -> eyre::Result<(ForkId, SharedBackend, EvmEnv<BlockT, TxT, HardforkT>)> {
        trace!(?fork, ?block, "rolling fork");
        let (sender, rx) = oneshot_channel();
        let req = Request::RollFork(fork, block, sender);
        self.handler
            .clone()
            .try_send(req)
            .map_err(|e| eyre::eyre!("{:?}", e))?;
        rx.recv()?
    }

    /// Returns the `EvmEnv<BlockT, TxT, HardforkT>` of the given fork, if any
    pub fn get_env(&self, fork: ForkId) -> eyre::Result<Option<EvmEnv<BlockT, TxT, HardforkT>>> {
        trace!(?fork, "getting env config");
        let (sender, rx) = oneshot_channel();
        let req = Request::GetEnv(fork, sender);
        self.handler
            .clone()
            .try_send(req)
            .map_err(|e| eyre::eyre!("{:?}", e))?;
        Ok(rx.recv()?)
    }

    /// Returns the corresponding fork if it exists
    ///
    /// Returns `None` if no matching fork backend is available.
    pub fn get_fork(&self, id: impl Into<ForkId>) -> eyre::Result<Option<SharedBackend>> {
        let id = id.into();
        trace!(?id, "get fork backend");
        let (sender, rx) = oneshot_channel();
        let req = Request::GetFork(id, sender);
        self.handler
            .clone()
            .try_send(req)
            .map_err(|e| eyre::eyre!("{:?}", e))?;
        Ok(rx.recv()?)
    }

    /// Returns the corresponding fork url if it exists
    ///
    /// Returns `None` if no matching fork is available.
    pub fn get_fork_url(&self, id: impl Into<ForkId>) -> eyre::Result<Option<String>> {
        let (sender, rx) = oneshot_channel();
        let req = Request::GetForkUrl(id.into(), sender);
        self.handler
            .clone()
            .try_send(req)
            .map_err(|e| eyre::eyre!("{:?}", e))?;
        Ok(rx.recv()?)
    }
}

type Handler = BackendHandler<Arc<RetryProvider>>;

type CreateFuture<BlockT, TxT, HardforkT> = Pin<
    Box<
        dyn Future<Output = eyre::Result<(ForkId, CreatedFork<BlockT, TxT, HardforkT>, Handler)>>
            + Send,
    >,
>;
type CreateSender<BlockT, TxT, HardforkT> =
    OneshotSender<eyre::Result<(ForkId, SharedBackend, EvmEnv<BlockT, TxT, HardforkT>)>>;
type GetEnvSender<BlockT, TxT, HardforkT> = OneshotSender<Option<EvmEnv<BlockT, TxT, HardforkT>>>;

/// Request that's send to the handler
#[derive(Debug)]
enum Request<BlockT, TxT, HardforkT> {
    /// Creates a new `ForkBackend`
    CreateFork(
        Box<CreateFork<BlockT, TxT, HardforkT>>,
        CreateSender<BlockT, TxT, HardforkT>,
    ),
    /// Returns the Fork backend for the `ForkId` if it exists
    GetFork(ForkId, OneshotSender<Option<SharedBackend>>),
    /// Adjusts the block that's being forked, by creating a new fork at the new
    /// block
    RollFork(ForkId, u64, CreateSender<BlockT, TxT, HardforkT>),
    /// Returns the environment of the fork
    GetEnv(ForkId, GetEnvSender<BlockT, TxT, HardforkT>),
    /// Shutdowns the entire `MultiForkHandler`, see `ShutDownMultiFork`
    ShutDown(OneshotSender<()>),
    /// Returns the Fork Url for the `ForkId` if it exists
    GetForkUrl(ForkId, OneshotSender<Option<String>>),
}

enum ForkTask<BlockT, TxT, HardforkT> {
    /// Contains the future that will establish a new fork
    Create(
        CreateFuture<BlockT, TxT, HardforkT>,
        ForkId,
        CreateSender<BlockT, TxT, HardforkT>,
        Vec<CreateSender<BlockT, TxT, HardforkT>>,
    ),
}

/// The type that manages connections in the background
#[must_use = "futures do nothing unless polled"]
pub struct MultiForkHandler<BlockT, TxT, HardforkT> {
    /// Incoming requests from the `MultiFork`.
    incoming: Fuse<Receiver<Request<BlockT, TxT, HardforkT>>>,

    /// All active handlers
    ///
    /// It's expected that this list will be rather small (<10)
    handlers: Vec<(ForkId, Handler)>,

    // tasks currently in progress
    pending_tasks: Vec<ForkTask<BlockT, TxT, HardforkT>>,

    /// All _unique_ forkids mapped to their corresponding backend.
    ///
    /// Note: The backend can be shared by multiple `ForkIds` if the target the
    /// same provider and block number.
    forks: HashMap<ForkId, CreatedFork<BlockT, TxT, HardforkT>>,

    /// Optional periodic interval to flush rpc cache
    flush_cache_interval: Option<tokio::time::Interval>,
}

// === impl MultiForkHandler ===

impl<BlockT: BlockEnvTr, TxT: TransactionEnvTr, HardforkT: HardforkTr>
    MultiForkHandler<BlockT, TxT, HardforkT>
{
    fn new(incoming: Receiver<Request<BlockT, TxT, HardforkT>>) -> Self {
        Self {
            incoming: incoming.fuse(),
            handlers: Vec::default(),
            pending_tasks: Vec::default(),
            forks: HashMap::default(),
            flush_cache_interval: None,
        }
    }

    /// Sets the interval after which all rpc caches should be flushed
    /// periodically
    pub fn set_flush_cache_interval(&mut self, period: Duration) -> &mut Self {
        self.flush_cache_interval = Some(tokio::time::interval_at(
            tokio::time::Instant::now() + period,
            period,
        ));
        self
    }

    /// Returns the list of additional senders of a matching task for the given
    /// id, if any.
    fn find_in_progress_task(
        &mut self,
        id: &ForkId,
    ) -> Option<&mut Vec<CreateSender<BlockT, TxT, HardforkT>>> {
        for task in self.pending_tasks.iter_mut() {
            #[allow(irrefutable_let_patterns)]
            if let ForkTask::Create(_, in_progress, _, additional) = task {
                if in_progress == id {
                    return Some(additional);
                }
            }
        }
        None
    }

    fn create_fork(
        &mut self,
        fork: CreateFork<BlockT, TxT, HardforkT>,
        sender: CreateSender<BlockT, TxT, HardforkT>,
    ) {
        let fork_id = ForkId::new(&fork.url, fork.evm_opts.fork_block_number);
        trace!(?fork_id, "created new forkId");

        // there could already be a task for the requested fork in progress
        if let Some(in_progress) = self.find_in_progress_task(&fork_id) {
            in_progress.push(sender);
            return;
        }

        // need to create a new fork
        let task = Box::pin(create_fork(fork));
        self.pending_tasks
            .push(ForkTask::Create(task, fork_id, sender, Vec::new()));
    }

    fn insert_new_fork(
        &mut self,
        fork_id: ForkId,
        fork: CreatedFork<BlockT, TxT, HardforkT>,
        sender: CreateSender<BlockT, TxT, HardforkT>,
        additional_senders: Vec<CreateSender<BlockT, TxT, HardforkT>>,
    ) {
        self.forks.insert(fork_id.clone(), fork.clone());
        let _ = sender.send(Ok((
            fork_id.clone(),
            fork.backend.clone(),
            fork.opts.env.clone(),
        )));

        // notify all additional senders and track unique forkIds
        for sender in additional_senders {
            let next_fork_id = fork.inc_senders(fork_id.clone());
            self.forks.insert(next_fork_id.clone(), fork.clone());
            let _ = sender.send(Ok((
                next_fork_id,
                fork.backend.clone(),
                fork.opts.env.clone(),
            )));
        }
    }

    fn on_request(&mut self, req: Request<BlockT, TxT, HardforkT>) {
        match req {
            Request::CreateFork(fork, sender) => self.create_fork(*fork, sender),
            Request::GetFork(fork_id, sender) => {
                let fork = self.forks.get(&fork_id).map(|f| f.backend.clone());
                let _ = sender.send(fork);
            }
            Request::RollFork(fork_id, block, sender) => {
                if let Some(fork) = self.forks.get(&fork_id) {
                    trace!(target: "fork::multi", "rolling {} to {}", fork_id, block);
                    let mut opts = fork.opts.clone();
                    opts.evm_opts.fork_block_number = Some(block);
                    self.create_fork(opts, sender);
                } else {
                    let _ = sender.send(Err(eyre::eyre!("No matching fork exits for {}", fork_id)));
                }
            }
            Request::GetEnv(fork_id, sender) => {
                let _ = sender.send(self.forks.get(&fork_id).map(|fork| fork.opts.env.clone()));
            }
            Request::ShutDown(sender) => {
                trace!(target: "fork::multi", "received shutdown signal");
                // we're emptying all fork backends, this way we ensure all caches get flushed
                self.forks.clear();
                self.handlers.clear();
                let _ = sender.send(());
            }
            Request::GetForkUrl(fork_id, sender) => {
                let fork = self.forks.get(&fork_id).map(|f| f.opts.url.clone());
                let _ = sender.send(fork);
            }
        }
    }
}

// Drives all handler to completion
// This future will finish once all underlying BackendHandler are completed
impl<BlockT: BlockEnvTr, TxT: TransactionEnvTr, HardforkT: HardforkTr> Future
    for MultiForkHandler<BlockT, TxT, HardforkT>
{
    type Output = ();

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let pin = self.get_mut();

        // receive new requests
        loop {
            match Pin::new(&mut pin.incoming).poll_next(cx) {
                Poll::Ready(Some(req)) => {
                    pin.on_request(req);
                }
                Poll::Ready(None) => {
                    // channel closed, but we still need to drive the fork handlers to completion
                    trace!(target: "fork::multi", "request channel closed");
                    break;
                }
                Poll::Pending => break,
            }
        }

        // advance all tasks
        for n in (0..pin.pending_tasks.len()).rev() {
            let task = pin.pending_tasks.swap_remove(n);
            match task {
                ForkTask::Create(mut fut, id, sender, additional_senders) => {
                    if let Poll::Ready(resp) = fut.poll_unpin(cx) {
                        match resp {
                            Ok((fork_id, fork, handler)) => {
                                if let Some(fork) = pin.forks.get(&fork_id).cloned() {
                                    pin.insert_new_fork(
                                        fork.inc_senders(fork_id),
                                        fork,
                                        sender,
                                        additional_senders,
                                    );
                                } else {
                                    pin.handlers.push((fork_id.clone(), handler));
                                    pin.insert_new_fork(fork_id, fork, sender, additional_senders);
                                }
                            }
                            Err(err) => {
                                let _ = sender.send(Err(eyre::eyre!("{err}")));
                                for sender in additional_senders {
                                    let _ = sender.send(Err(eyre::eyre!("{err}")));
                                }
                            }
                        }
                    } else {
                        pin.pending_tasks.push(ForkTask::Create(
                            fut,
                            id,
                            sender,
                            additional_senders,
                        ));
                    }
                }
            }
        }

        // advance all handlers
        for n in (0..pin.handlers.len()).rev() {
            let (id, mut handler) = pin.handlers.swap_remove(n);
            match handler.poll_unpin(cx) {
                Poll::Ready(_) => {
                    trace!(target: "fork::multi", "fork {:?} completed", id);
                }
                Poll::Pending => {
                    pin.handlers.push((id, handler));
                }
            }
        }

        if pin.handlers.is_empty() && pin.incoming.is_done() {
            trace!(target: "fork::multi", "completed");
            return Poll::Ready(());
        }

        // periodically flush cached RPC state
        if pin
            .flush_cache_interval
            .as_mut()
            .map(|interval| interval.poll_tick(cx).is_ready())
            .unwrap_or_default()
            && !pin.forks.is_empty()
        {
            trace!(target: "fork::multi", "tick flushing caches");
            let forks = pin
                .forks
                .values()
                .map(|f| f.backend.clone())
                .collect::<Vec<_>>();
            // flush this on new thread to not block here
            std::thread::Builder::new()
                .name("flusher".into())
                .spawn(move || {
                    forks.into_iter().for_each(|fork| fork.flush_cache());
                })
                .expect("failed to spawn thread");
        }

        Poll::Pending
    }
}

/// Tracks the created Fork
#[derive(Debug, Clone)]
struct CreatedFork<BlockT, TxT, HardforkT> {
    /// How the fork was initially created
    opts: CreateFork<BlockT, TxT, HardforkT>,
    /// Copy of the sender
    backend: SharedBackend,
    /// How many consumers there are, since a `SharedBacked` can be used by
    /// multiple consumers
    num_senders: Arc<AtomicUsize>,
}

// === impl CreatedFork ===

impl<BlockT: BlockEnvTr, TxT: TransactionEnvTr, HardforkT: HardforkTr>
    CreatedFork<BlockT, TxT, HardforkT>
{
    pub fn new(opts: CreateFork<BlockT, TxT, HardforkT>, backend: SharedBackend) -> Self {
        Self {
            opts,
            backend,
            num_senders: Arc::new(AtomicUsize::new(1)),
        }
    }

    /// Increment senders and return unique identifier of the fork
    fn inc_senders(&self, fork_id: ForkId) -> ForkId {
        format!(
            "{}-{}",
            fork_id.as_str(),
            self.num_senders
                .fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        )
        .into()
    }
}

/// A type that's used to signaling the `MultiForkHandler` when it's time to
/// shut down.
///
/// This is essentially a sync on drop, so that the `MultiForkHandler` can flush
/// all rpc cashes
///
/// This type intentionally does not implement `Clone` since it's intended that
/// there's only once instance.
#[derive(Debug)]
struct ShutDownMultiFork<BlockT, TxT, HardforkT> {
    handler: Option<Sender<Request<BlockT, TxT, HardforkT>>>,
}

impl<BlockT, TxT, HardforkT> Drop for ShutDownMultiFork<BlockT, TxT, HardforkT> {
    fn drop(&mut self) {
        trace!(target: "fork::multi", "initiating shutdown");
        let (sender, rx) = oneshot_channel();
        let req = Request::ShutDown(sender);
        if let Some(mut handler) = self.handler.take() {
            if handler.try_send(req).is_ok() {
                let _ = rx.recv();
                trace!(target: "fork::cache", "multifork backend shutdown");
            }
        }
    }
}

/// Creates a new fork
///
/// This will establish a new `Provider` to the endpoint and return the Fork
/// Backend
async fn create_fork<BlockT: BlockEnvTr, TxT: TransactionEnvTr, HardforkT: HardforkTr>(
    mut fork: CreateFork<BlockT, TxT, HardforkT>,
) -> eyre::Result<(ForkId, CreatedFork<BlockT, TxT, HardforkT>, Handler)> {
    let provider = Arc::new(
        ProviderBuilder::new(fork.url.as_str())
            .maybe_max_retry(fork.evm_opts.fork_retries)
            .maybe_initial_backoff(fork.evm_opts.fork_retry_backoff)
            .compute_units_per_second(fork.evm_opts.get_compute_units_per_second())
            .build()?,
    );

    // initialise the fork environment
    let (env, block) = fork.evm_opts.fork_evm_env(&fork.url).await?;
    fork.env = env;
    let meta = BlockchainDbMeta::new(fork.env.block.clone().into(), fork.url.clone());

    // we need to use the block number from the block because the env's number can
    // be different on some L2s (e.g. Arbitrum).
    let number = block.header().number();

    let cache_path = fork.block_cache_dir(fork.env.cfg.chain_id, number);

    let db = BlockchainDb::new(meta, cache_path);
    let (backend, handler) = SharedBackend::new(provider, db, Some(number.into()));
    let fork = CreatedFork::new(fork, backend);
    let fork_id = ForkId::new(&fork.opts.url, number.into());

    Ok((fork_id, fork, handler))
}
