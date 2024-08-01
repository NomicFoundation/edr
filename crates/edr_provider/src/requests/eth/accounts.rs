use core::fmt::Debug;

use edr_eth::Address;
use edr_evm::chain_spec::ChainSpec;

use crate::{data::ProviderData, time::TimeSinceEpoch, ProviderError};

/// `require_canonical`: whether the server should additionally raise a JSON-RPC
/// error if the block is not in the canonical chain
pub fn handle_accounts_request<
    ChainSpecT: ChainSpec<Hardfork: Debug>,
    LoggerErrorT: Debug,
    TimerT: Clone + TimeSinceEpoch,
>(
    data: &ProviderData<ChainSpecT, LoggerErrorT, TimerT>,
) -> Result<Vec<Address>, ProviderError<ChainSpecT, LoggerErrorT>> {
    Ok(data.accounts().copied().collect())
}
