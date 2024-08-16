use edr_eth::Address;

use crate::{data::ProviderData, time::TimeSinceEpoch, ProviderError};

pub fn handle_impersonate_account_request<TimerT: Clone + TimeSinceEpoch>(
    data: &mut ProviderData<TimerT>,
    address: Address,
) -> Result<bool, ProviderError> {
    data.impersonate_account(address);

    Ok(true)
}

pub fn handle_stop_impersonating_account_request<TimerT: Clone + TimeSinceEpoch>(
    data: &mut ProviderData<TimerT>,
    address: Address,
) -> Result<bool, ProviderError> {
    Ok(data.stop_impersonating_account(address))
}
