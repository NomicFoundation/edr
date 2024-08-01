use core::fmt::Debug;

use edr_eth::{Address, U256, U64};
use edr_evm::chain_spec::{ChainSpec, SyncChainSpec};

use crate::{data::ProviderData, time::TimeSinceEpoch, ProviderError};

pub fn handle_blob_base_fee<
    ChainSpecT: SyncChainSpec<Hardfork: Debug>,
    LoggerErrorT: Debug,
    TimerT: Clone + TimeSinceEpoch,
>(
    data: &ProviderData<ChainSpecT, LoggerErrorT, TimerT>,
) -> Result<U256, ProviderError<ChainSpecT, LoggerErrorT>> {
    let base_fee = data.next_block_base_fee_per_blob_gas()?.unwrap_or_default();

    Ok(base_fee)
}

pub fn handle_gas_price<
    ChainSpecT: SyncChainSpec<Hardfork: Debug>,
    LoggerErrorT: Debug,
    TimerT: Clone + TimeSinceEpoch,
>(
    data: &ProviderData<ChainSpecT, LoggerErrorT, TimerT>,
) -> Result<U256, ProviderError<ChainSpecT, LoggerErrorT>> {
    data.gas_price()
}

pub fn handle_coinbase_request<
    ChainSpecT: ChainSpec<Hardfork: Debug>,
    LoggerErrorT: Debug,
    TimerT: Clone + TimeSinceEpoch,
>(
    data: &ProviderData<ChainSpecT, LoggerErrorT, TimerT>,
) -> Result<Address, ProviderError<ChainSpecT, LoggerErrorT>> {
    Ok(data.coinbase())
}

pub fn handle_max_priority_fee_per_gas<
    ChainSpecT: ChainSpec<Hardfork: Debug>,
    LoggerErrorT: Debug,
>() -> Result<U256, ProviderError<ChainSpecT, LoggerErrorT>> {
    // 1 gwei
    Ok(U256::from(1_000_000_000))
}

pub fn handle_mining<ChainSpecT: ChainSpec<Hardfork: Debug>, LoggerErrorT: Debug>(
) -> Result<bool, ProviderError<ChainSpecT, LoggerErrorT>> {
    Ok(false)
}

pub fn handle_net_listening_request<ChainSpecT: ChainSpec<Hardfork: Debug>, LoggerErrorT: Debug>(
) -> Result<bool, ProviderError<ChainSpecT, LoggerErrorT>> {
    Ok(true)
}

pub fn handle_net_peer_count_request<
    ChainSpecT: ChainSpec<Hardfork: Debug>,
    LoggerErrorT: Debug,
>() -> Result<U64, ProviderError<ChainSpecT, LoggerErrorT>> {
    Ok(U64::from(0))
}

pub fn handle_net_version_request<
    ChainSpecT: ChainSpec<Hardfork: Debug>,
    LoggerErrorT: Debug,
    TimerT: Clone + TimeSinceEpoch,
>(
    data: &ProviderData<ChainSpecT, LoggerErrorT, TimerT>,
) -> Result<String, ProviderError<ChainSpecT, LoggerErrorT>> {
    Ok(data.network_id())
}

pub fn handle_syncing<ChainSpecT: ChainSpec<Hardfork: Debug>, LoggerErrorT: Debug>(
) -> Result<bool, ProviderError<ChainSpecT, LoggerErrorT>> {
    Ok(false)
}
