use edr_eth::{Address, U256, U64};

use crate::{data::ProviderData, time::TimeSinceEpoch, ProviderError};

pub fn handle_blob_base_fee<TimerT: Clone + TimeSinceEpoch>(
    data: &ProviderData<TimerT>,
) -> Result<U256, ProviderError> {
    let base_fee = data.next_block_base_fee_per_blob_gas()?.unwrap_or_default();

    Ok(base_fee)
}

pub fn handle_gas_price<TimerT: Clone + TimeSinceEpoch>(
    data: &ProviderData<TimerT>,
) -> Result<U256, ProviderError> {
    data.gas_price()
}

pub fn handle_coinbase_request<TimerT: Clone + TimeSinceEpoch>(
    data: &ProviderData<TimerT>,
) -> Result<Address, ProviderError> {
    Ok(data.coinbase())
}

pub fn handle_max_priority_fee_per_gas() -> Result<U256, ProviderError> {
    // 1 gwei
    Ok(U256::from(1_000_000_000))
}

pub fn handle_mining() -> Result<bool, ProviderError> {
    Ok(false)
}

pub fn handle_net_listening_request() -> Result<bool, ProviderError> {
    Ok(true)
}

pub fn handle_net_peer_count_request() -> Result<U64, ProviderError> {
    Ok(U64::from(0))
}

pub fn handle_net_version_request<TimerT: Clone + TimeSinceEpoch>(
    data: &ProviderData<TimerT>,
) -> Result<String, ProviderError> {
    Ok(data.network_id())
}

pub fn handle_syncing() -> Result<bool, ProviderError> {
    Ok(false)
}
