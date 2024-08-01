use core::fmt::Debug;

use edr_evm::chain_spec::ChainSpec;

use crate::{data::ProviderData, time::TimeSinceEpoch, ProviderError};

pub fn handle_set_logging_enabled_request<
    ChainSpecT: ChainSpec<Hardfork: Debug>,
    LoggerErrorT: Debug,
    TimerT: Clone + TimeSinceEpoch,
>(
    data: &mut ProviderData<ChainSpecT, LoggerErrorT, TimerT>,
    is_enabled: bool,
) -> Result<bool, ProviderError<ChainSpecT, LoggerErrorT>> {
    data.logger_mut().set_is_enabled(is_enabled);
    Ok(true)
}
