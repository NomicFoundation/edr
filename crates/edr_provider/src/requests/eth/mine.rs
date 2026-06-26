use crate::{
    config::IntervalConfig, data::ProviderData, error::ProviderErrorForChainSpec, requests,
    spec::SyncProviderSpec, time::TimeSinceEpoch,
};

pub fn handle_set_interval_mining<
    ChainSpecT: SyncProviderSpec<TimerT, SignedTransaction: Default>,
    TimerT: Clone + TimeSinceEpoch,
>(
    data: &mut ProviderData<ChainSpecT, TimerT>,
    config: requests::IntervalConfig,
) -> Result<bool, ProviderErrorForChainSpec<ChainSpecT>> {
    let config: Option<IntervalConfig> = config.try_into()?;
    data.set_interval_config(config);

    Ok(true)
}
