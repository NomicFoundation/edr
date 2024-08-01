use core::fmt::Debug;

use edr_eth::{Address, Bytes, U256};
use edr_evm::chain_spec::SyncChainSpec;

use crate::{data::ProviderData, time::TimeSinceEpoch, ProviderError};

pub fn handle_set_balance<
    ChainSpecT: SyncChainSpec<Hardfork: Debug>,
    LoggerErrorT: Debug,
    TimerT: Clone + TimeSinceEpoch,
>(
    data: &mut ProviderData<ChainSpecT, LoggerErrorT, TimerT>,
    address: Address,
    balance: U256,
) -> Result<bool, ProviderError<ChainSpecT, LoggerErrorT>> {
    data.set_balance(address, balance)?;

    Ok(true)
}

pub fn handle_set_code<
    ChainSpecT: SyncChainSpec<Hardfork: Debug>,
    LoggerErrorT: Debug,
    TimerT: Clone + TimeSinceEpoch,
>(
    data: &mut ProviderData<ChainSpecT, LoggerErrorT, TimerT>,
    address: Address,
    code: Bytes,
) -> Result<bool, ProviderError<ChainSpecT, LoggerErrorT>> {
    data.set_code(address, code)?;

    Ok(true)
}

pub fn handle_set_nonce<
    ChainSpecT: SyncChainSpec<Hardfork: Debug>,
    LoggerErrorT: Debug,
    TimerT: Clone + TimeSinceEpoch,
>(
    data: &mut ProviderData<ChainSpecT, LoggerErrorT, TimerT>,
    address: Address,
    nonce: u64,
) -> Result<bool, ProviderError<ChainSpecT, LoggerErrorT>> {
    data.set_nonce(address, nonce)?;

    Ok(true)
}

pub fn handle_set_storage_at<
    ChainSpecT: SyncChainSpec<Hardfork: Debug>,
    LoggerErrorT: Debug,
    TimerT: Clone + TimeSinceEpoch,
>(
    data: &mut ProviderData<ChainSpecT, LoggerErrorT, TimerT>,
    address: Address,
    index: U256,
    value: U256,
) -> Result<bool, ProviderError<ChainSpecT, LoggerErrorT>> {
    data.set_account_storage_slot(address, index, value)?;

    Ok(true)
}
