use std::{marker::PhantomData, sync::Arc};

use tokio::{
    runtime,
    sync::{oneshot, Mutex},
    task::JoinHandle,
    time::Instant,
};

use crate::{
    config::IntervalConfig, data::ProviderData, error::ProviderErrorForChainSpec,
    spec::SyncProviderSpec, time::TimeSinceEpoch, ProviderSpec,
};

/// Type for interval mining on a separate thread.
pub struct IntervalMiner<ChainSpecT: ProviderSpec<TimerT>, TimerT: Clone + TimeSinceEpoch> {
    inner: Option<Inner<ChainSpecT, TimerT>>,
    runtime: runtime::Handle,
}

/// Inner type for interval mining on a separate thread, required for
/// implementation of `Drop`.
struct Inner<ChainSpecT: ProviderSpec<TimerT>, TimerT: Clone + TimeSinceEpoch> {
    cancellation_sender: oneshot::Sender<()>,
    background_task: JoinHandle<Result<(), ProviderErrorForChainSpec<ChainSpecT>>>,
    // Receives a signal when the background task exits (sender is dropped by
    // the task on return). Used in Drop to wait without polling.
    task_exited_rx: std::sync::mpsc::Receiver<()>,
    _phantom: PhantomData<fn() -> TimerT>,
}

impl<
        ChainSpecT: SyncProviderSpec<TimerT, SignedTransaction: Default>,
        TimerT: Clone + TimeSinceEpoch,
    > IntervalMiner<ChainSpecT, TimerT>
{
    pub fn new(
        runtime: runtime::Handle,
        config: IntervalConfig,
        data: Arc<Mutex<ProviderData<ChainSpecT, TimerT>>>,
    ) -> Self {
        let (cancellation_sender, cancellation_receiver) = oneshot::channel();
        let (task_exited_tx, task_exited_rx) = std::sync::mpsc::channel::<()>();
        let background_task = runtime.spawn(async move {
            interval_mining_loop(config, data, cancellation_receiver, task_exited_tx).await
        });

        Self {
            inner: Some(Inner {
                cancellation_sender,
                background_task,
                task_exited_rx,
                _phantom: PhantomData,
            }),
            runtime,
        }
    }
}

#[cfg_attr(feature = "tracing", tracing::instrument(skip_all))]
async fn interval_mining_loop<
    ChainSpecT: SyncProviderSpec<TimerT, SignedTransaction: Default>,
    TimerT: Clone + TimeSinceEpoch,
>(
    config: IntervalConfig,
    data: Arc<Mutex<ProviderData<ChainSpecT, TimerT>>>,
    mut cancellation_receiver: oneshot::Receiver<()>,
    // Dropped when the loop exits, waking the Drop-side recv_timeout.
    _task_exited_tx: std::sync::mpsc::Sender<()>,
) -> Result<(), ProviderErrorForChainSpec<ChainSpecT>> {
    let mut now = Instant::now();
    loop {
        let delay = config.generate_interval();
        let deadline = now + std::time::Duration::from_millis(delay);

        tokio::select! {
            _ = &mut cancellation_receiver => return Ok(()),
            _ = tokio::time::sleep_until(deadline) => {
                tokio::select! {
                    // Check whether the interval miner needs to be destroyed
                    _ = &mut cancellation_receiver => return Ok(()),
                    mut data = data.lock() => {
                        now = Instant::now();

                        if let Err(error) = data.interval_mine() {
                            log::error!("Unexpected error while performing interval mining: {error}");
                            return Err(error);
                        }

                        Result::<(), ProviderErrorForChainSpec<ChainSpecT>>::Ok(())
                    }
                }
            },
        }?;
    }
}

impl<ChainSpecT: ProviderSpec<TimerT>, TimerT: Clone + TimeSinceEpoch> Drop
    for IntervalMiner<ChainSpecT, TimerT>
{
    #[cfg_attr(feature = "tracing", tracing::instrument(skip_all))]
    fn drop(&mut self) {
        if let Some(Inner {
            cancellation_sender,
            background_task: task,
            task_exited_rx,
            _phantom,
        }) = self.inner.take()
        {
            if cancellation_sender.send(()).is_ok() {
                // task_exited_tx is dropped when interval_mining_loop returns,
                // waking recv_timeout immediately in the normal case.
                // In the deadlock case recv_timeout returns Err(Timeout) after 5s.
                use std::sync::mpsc::RecvTimeoutError;
                match task_exited_rx.recv_timeout(std::time::Duration::from_secs(5)) {
                    Err(RecvTimeoutError::Timeout) => {
                        log::warn!(
                            target: "edr_provider::interval_miner",
                            "IntervalMiner::Drop timed out (TSFN deadlock suspected)"
                        );
                        eprintln!(
                            "[edr_provider] WARNING: IntervalMiner::Drop timed out after 5s \
                             (TSFN deadlock suspected)"
                        );
                        // task is dropped here — detaches without aborting
                    }
                    _ => {
                        // Task exited normally; block_on polls once and returns
                        // immediately since the task is already done.
                        let _ = tokio::task::block_in_place(|| self.runtime.block_on(task))
                            .expect("interval mining task panicked");
                    }
                }
            } else {
                log::debug!(
                    "Failed to send cancellation signal to interval mining task. The runtime must have already terminated."
                );
            }
        }
    }
}
