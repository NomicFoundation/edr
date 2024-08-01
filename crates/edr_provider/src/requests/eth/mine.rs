use core::fmt::Debug;
use std::sync::Arc;

use edr_evm::chain_spec::ChainSpec;
use tokio::{runtime, sync::Mutex};

use crate::{
    data::ProviderData, interval::IntervalMiner, requests, time::TimeSinceEpoch, IntervalConfig,
    ProviderError,
};

pub fn handle_set_interval_mining<
    ChainSpecT: ChainSpec<Hardfork: Debug>,
    LoggerErrorT: Debug + Send + Sync + 'static,
    TimerT: Clone + TimeSinceEpoch,
>(
    data: Arc<Mutex<ProviderData<ChainSpecT, LoggerErrorT, TimerT>>>,
    interval_miner: &mut Option<IntervalMiner<ChainSpecT, LoggerErrorT>>,
    runtime: runtime::Handle,
    config: requests::IntervalConfig,
) -> Result<bool, ProviderError<ChainSpecT, LoggerErrorT>> {
    let config: Option<IntervalConfig> = config.try_into()?;
    *interval_miner = config.map(|config| IntervalMiner::new(runtime, config, data.clone()));

    Ok(true)
}
