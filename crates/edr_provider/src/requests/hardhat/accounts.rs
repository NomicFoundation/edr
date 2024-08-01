use core::fmt::Debug;

use edr_eth::Address;
use edr_evm::chain_spec::ChainSpec;

use crate::{data::ProviderData, time::TimeSinceEpoch, ProviderError};

pub fn handle_impersonate_account_request<
    ChainSpecT: ChainSpec<Hardfork: Debug>,
    LoggerErrorT: Debug,
    TimerT: Clone + TimeSinceEpoch,
>(
    data: &mut ProviderData<ChainSpecT, LoggerErrorT, TimerT>,
    address: Address,
) -> Result<bool, ProviderError<ChainSpecT, LoggerErrorT>> {
    data.impersonate_account(address);

    Ok(true)
}

pub fn handle_stop_impersonating_account_request<
    ChainSpecT: ChainSpec<Hardfork: Debug>,
    LoggerErrorT: Debug,
    TimerT: Clone + TimeSinceEpoch,
>(
    data: &mut ProviderData<ChainSpecT, LoggerErrorT, TimerT>,
    address: Address,
) -> Result<bool, ProviderError<ChainSpecT, LoggerErrorT>> {
    Ok(data.stop_impersonating_account(address))
}
