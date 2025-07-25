use std::{marker::PhantomData, sync::Arc};

use edr_eth::{l1, transaction::TransactionValidation};
use edr_evm::spec::RuntimeSpec;
use tokio::{
    runtime,
    sync::{oneshot, Mutex},
    task::JoinHandle,
    time::Instant,
};

use crate::{
    data::ProviderData, error::ProviderErrorForChainSpec, spec::SyncProviderSpec,
    time::TimeSinceEpoch, IntervalConfig,
};

/// Type for interval mining on a separate thread.
pub struct IntervalMiner<ChainSpecT: RuntimeSpec, TimerT> {
    inner: Option<Inner<ChainSpecT>>,
    runtime: runtime::Handle,
    phantom: PhantomData<TimerT>,
}

/// Inner type for interval mining on a separate thread, required for
/// implementation of `Drop`.
struct Inner<ChainSpecT: RuntimeSpec> {
    cancellation_sender: oneshot::Sender<()>,
    background_task: JoinHandle<Result<(), ProviderErrorForChainSpec<ChainSpecT>>>,
}

impl<
        ChainSpecT: SyncProviderSpec<
            TimerT,
            BlockEnv: Default,
            SignedTransaction: Default
                                   + TransactionValidation<
                ValidationError: From<l1::InvalidTransaction> + PartialEq,
            >,
        >,
        TimerT: Clone + TimeSinceEpoch,
    > IntervalMiner<ChainSpecT, TimerT>
{
    pub fn new(
        runtime: runtime::Handle,
        config: IntervalConfig,
        data: Arc<Mutex<ProviderData<ChainSpecT, TimerT>>>,
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
            phantom: PhantomData,
        }
    }
}

#[cfg_attr(feature = "tracing", tracing::instrument(skip_all))]
async fn interval_mining_loop<
    ChainSpecT: SyncProviderSpec<
        TimerT,
        BlockEnv: Default,
        SignedTransaction: Default
                               + TransactionValidation<
            ValidationError: From<l1::InvalidTransaction> + PartialEq,
        >,
    >,
    TimerT: Clone + TimeSinceEpoch,
>(
    config: IntervalConfig,
    data: Arc<Mutex<ProviderData<ChainSpecT, TimerT>>>,
    mut cancellation_receiver: oneshot::Receiver<()>,
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

impl<ChainSpecT: RuntimeSpec, TimerT> Drop for IntervalMiner<ChainSpecT, TimerT> {
    #[cfg_attr(feature = "tracing", tracing::instrument(skip_all))]
    fn drop(&mut self) {
        if let Some(Inner {
            cancellation_sender,
            background_task: task,
        }) = self.inner.take()
        {
            if let Ok(()) = cancellation_sender.send(()) {
                let _result = tokio::task::block_in_place(move || self.runtime.block_on(task))
                    .expect("Failed to join interval mininig task");
            } else {
                log::debug!(
                    "Failed to send cancellation signal to interval mining task. The runtime must have already terminated."
                );
            }
        }
    }
}
