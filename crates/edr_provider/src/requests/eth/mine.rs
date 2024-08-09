use std::sync::Arc;

use tokio::{runtime, sync::Mutex};

use crate::{
    data::ProviderData, interval::IntervalMiner, requests, time::TimeSinceEpoch, IntervalConfig,
    ProviderError,
};

pub fn handle_set_interval_mining<TimerT: Clone + TimeSinceEpoch>(
    data: Arc<Mutex<ProviderData<TimerT>>>,
    interval_miner: &mut Option<IntervalMiner>,
    runtime: runtime::Handle,
    config: requests::IntervalConfig,
) -> Result<bool, ProviderError> {
    let config: Option<IntervalConfig> = config.try_into()?;
    *interval_miner = config.map(|config| IntervalMiner::new(runtime, config, data.clone()));

    Ok(true)
}
