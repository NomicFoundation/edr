use edr_eth::Address;

use crate::{
    ProviderErrorForChainSpec, data::ProviderData, spec::ProviderSpec, time::TimeSinceEpoch,
};

pub fn handle_impersonate_account_request<
    ChainSpecT: ProviderSpec<TimerT>,
    TimerT: Clone + TimeSinceEpoch,
>(
    data: &mut ProviderData<ChainSpecT, TimerT>,
    address: Address,
) -> Result<bool, ProviderErrorForChainSpec<ChainSpecT>> {
    data.impersonate_account(address);

    Ok(true)
}

pub fn handle_stop_impersonating_account_request<
    ChainSpecT: ProviderSpec<TimerT>,
    TimerT: Clone + TimeSinceEpoch,
>(
    data: &mut ProviderData<ChainSpecT, TimerT>,
    address: Address,
) -> Result<bool, ProviderErrorForChainSpec<ChainSpecT>> {
    Ok(data.stop_impersonating_account(address))
}
