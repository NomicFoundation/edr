use edr_primitives::{Address, Bytes, U256};
use edr_state_dump::StateDump;

use crate::{
    data::ProviderData, spec::SyncProviderSpec, time::TimeSinceEpoch, ProviderErrorForChainSpec,
};

pub fn handle_dump_state<ChainSpecT: SyncProviderSpec<TimerT>, TimerT: Clone + TimeSinceEpoch>(
    data: &mut ProviderData<ChainSpecT, TimerT>,
) -> Result<Bytes, ProviderErrorForChainSpec<ChainSpecT>> {
    let state_dump = data.dump_state()?;

    Ok(Bytes::from(state_dump.encode()))
}

pub fn handle_load_state<ChainSpecT: SyncProviderSpec<TimerT>, TimerT: Clone + TimeSinceEpoch>(
    data: &mut ProviderData<ChainSpecT, TimerT>,
    state_dump: Bytes,
) -> Result<bool, ProviderErrorForChainSpec<ChainSpecT>> {
    let state_dump = StateDump::decode(&state_dump)?;
    data.load_state(state_dump)?;

    Ok(true)
}

pub fn handle_set_balance<ChainSpecT: SyncProviderSpec<TimerT>, TimerT: Clone + TimeSinceEpoch>(
    data: &mut ProviderData<ChainSpecT, TimerT>,
    address: Address,
    balance: U256,
) -> Result<bool, ProviderErrorForChainSpec<ChainSpecT>> {
    data.set_balance(address, balance)?;

    Ok(true)
}

pub fn handle_set_code<ChainSpecT: SyncProviderSpec<TimerT>, TimerT: Clone + TimeSinceEpoch>(
    data: &mut ProviderData<ChainSpecT, TimerT>,
    address: Address,
    code: Bytes,
) -> Result<bool, ProviderErrorForChainSpec<ChainSpecT>> {
    data.set_code(address, code)?;

    Ok(true)
}

pub fn handle_set_nonce<ChainSpecT: SyncProviderSpec<TimerT>, TimerT: Clone + TimeSinceEpoch>(
    data: &mut ProviderData<ChainSpecT, TimerT>,
    address: Address,
    nonce: u64,
) -> Result<bool, ProviderErrorForChainSpec<ChainSpecT>> {
    data.set_nonce(address, nonce)?;

    Ok(true)
}

pub fn handle_set_storage_at<
    ChainSpecT: SyncProviderSpec<TimerT>,
    TimerT: Clone + TimeSinceEpoch,
>(
    data: &mut ProviderData<ChainSpecT, TimerT>,
    address: Address,
    index: U256,
    value: U256,
) -> Result<bool, ProviderErrorForChainSpec<ChainSpecT>> {
    data.set_account_storage_slot(address, index, value)?;

    Ok(true)
}
