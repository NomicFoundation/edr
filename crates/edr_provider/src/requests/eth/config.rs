use edr_blockchain_api::r#dyn::DynBlockchainError;
use edr_primitives::{Address, U256};

use crate::{
    data::ProviderData,
    spec::{ProviderSpec, SyncProviderSpec},
    time::TimeSinceEpoch,
};

pub fn handle_blob_base_fee<
    ChainSpecT: SyncProviderSpec<TimerT>,
    TimerT: Clone + TimeSinceEpoch,
>(
    data: &ProviderData<ChainSpecT, TimerT>,
) -> Result<U256, DynBlockchainError> {
    let base_fee = data.next_block_base_fee_per_blob_gas()?.unwrap_or_default();

    Ok(U256::from(base_fee))
}

pub fn handle_gas_price<ChainSpecT: SyncProviderSpec<TimerT>, TimerT: Clone + TimeSinceEpoch>(
    data: &ProviderData<ChainSpecT, TimerT>,
) -> Result<U256, DynBlockchainError> {
    data.gas_price().map(U256::from)
}

pub fn handle_coinbase_request<ChainSpecT: ProviderSpec<TimerT>, TimerT: Clone + TimeSinceEpoch>(
    data: &ProviderData<ChainSpecT, TimerT>,
) -> Address {
    data.coinbase()
}

pub fn handle_max_priority_fee_per_gas() -> U256 {
    // 1 gwei
    U256::from(1_000_000_000)
}

pub fn handle_net_version_request<
    ChainSpecT: ProviderSpec<TimerT>,
    TimerT: Clone + TimeSinceEpoch,
>(
    data: &ProviderData<ChainSpecT, TimerT>,
) -> String {
    data.network_id()
}

pub fn handle_syncing() -> bool {
    false
}
