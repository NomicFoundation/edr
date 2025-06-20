use crate::{
    data::ProviderData, spec::ProviderSpec, time::TimeSinceEpoch, ProviderErrorForChainSpec,
};

pub fn handle_set_logging_enabled_request<
    ChainSpecT: ProviderSpec<TimerT>,
    TimerT: Clone + TimeSinceEpoch,
>(
    data: &mut ProviderData<ChainSpecT, TimerT>,
    is_enabled: bool,
) -> Result<bool, ProviderErrorForChainSpec<ChainSpecT>> {
    data.logger_mut().set_is_enabled(is_enabled);
    Ok(true)
}
