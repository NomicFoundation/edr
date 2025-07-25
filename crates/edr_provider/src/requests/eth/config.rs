use edr_eth::{Address, U256};
use edr_evm::spec::RuntimeSpec;

use crate::{
    data::ProviderData,
    spec::{ProviderSpec, SyncProviderSpec},
    time::TimeSinceEpoch,
    ProviderErrorForChainSpec,
};

pub fn handle_blob_base_fee<
    ChainSpecT: SyncProviderSpec<TimerT>,
    TimerT: Clone + TimeSinceEpoch,
>(
    data: &ProviderData<ChainSpecT, TimerT>,
) -> Result<U256, ProviderErrorForChainSpec<ChainSpecT>> {
    let base_fee = data.next_block_base_fee_per_blob_gas()?.unwrap_or_default();

    Ok(U256::from(base_fee))
}

pub fn handle_gas_price<ChainSpecT: SyncProviderSpec<TimerT>, TimerT: Clone + TimeSinceEpoch>(
    data: &ProviderData<ChainSpecT, TimerT>,
) -> Result<U256, ProviderErrorForChainSpec<ChainSpecT>> {
    data.gas_price().map(U256::from)
}

pub fn handle_coinbase_request<ChainSpecT: ProviderSpec<TimerT>, TimerT: Clone + TimeSinceEpoch>(
    data: &ProviderData<ChainSpecT, TimerT>,
) -> Result<Address, ProviderErrorForChainSpec<ChainSpecT>> {
    Ok(data.coinbase())
}

pub fn handle_max_priority_fee_per_gas<ChainSpecT: RuntimeSpec>(
) -> Result<U256, ProviderErrorForChainSpec<ChainSpecT>> {
    // 1 gwei
    Ok(U256::from(1_000_000_000))
}

pub fn handle_net_version_request<
    ChainSpecT: ProviderSpec<TimerT>,
    TimerT: Clone + TimeSinceEpoch,
>(
    data: &ProviderData<ChainSpecT, TimerT>,
) -> Result<String, ProviderErrorForChainSpec<ChainSpecT>> {
    Ok(data.network_id())
}

pub fn handle_syncing<ChainSpecT: RuntimeSpec>(
) -> Result<bool, ProviderErrorForChainSpec<ChainSpecT>> {
    Ok(false)
}
