use std::sync::Arc;

use tokio::{
    runtime,
    sync::{oneshot, Mutex},
    task::JoinHandle,
    time::Instant,
};

use crate::{data::ProviderData, time::TimeSinceEpoch, IntervalConfig, ProviderError};

/// Type for interval mining on a separate thread.
pub struct IntervalMiner {
    inner: Option<Inner>,
    runtime: runtime::Handle,
}

/// Inner type for interval mining on a separate thread, required for
/// implementation of `Drop`.
struct Inner {
    cancellation_sender: oneshot::Sender<()>,
    background_task: JoinHandle<Result<(), ProviderError>>,
}

impl IntervalMiner {
    pub fn new<TimerT: Clone + TimeSinceEpoch>(
        runtime: runtime::Handle,
        config: IntervalConfig,
        data: Arc<Mutex<ProviderData<TimerT>>>,
    ) -> Self {
        let (cancellation_sender, cancellation_receiver) = oneshot::channel();
        let background_task = runtime
            .spawn(async move { interval_mining_loop(config, data, cancellation_receiver).await });

        Self {
            inner: Some(Inner {
                cancellation_sender,
                background_task,
            }),
            runtime,
        }
    }
}

#[cfg_attr(feature = "tracing", tracing::instrument(skip_all))]
async fn interval_mining_loop<TimerT: Clone + TimeSinceEpoch>(
    config: IntervalConfig,
    data: Arc<Mutex<ProviderData<TimerT>>>,
    mut cancellation_receiver: oneshot::Receiver<()>,
) -> Result<(), ProviderError> {
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

                        Result::<(), ProviderError>::Ok(())
                    }
                }
            },
        }?;
    }
}

impl Drop for IntervalMiner {
    #[cfg_attr(feature = "tracing", tracing::instrument(skip_all))]
    fn drop(&mut self) {
        if let Some(Inner {
            cancellation_sender,
            background_task: task,
        }) = self.inner.take()
        {
            cancellation_sender
                .send(())
                .expect("Failed to send cancellation signal");

            let _result = tokio::task::block_in_place(move || self.runtime.block_on(task))
                .expect("Failed to join interval mininig task");
        }
    }
}
