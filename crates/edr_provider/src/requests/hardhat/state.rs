use edr_eth::{Address, Bytes, U256};

use crate::{data::ProviderData, time::TimeSinceEpoch, ProviderError};

pub fn handle_set_balance<TimerT: Clone + TimeSinceEpoch>(
    data: &mut ProviderData<TimerT>,
    address: Address,
    balance: U256,
) -> Result<bool, ProviderError> {
    data.set_balance(address, balance)?;

    Ok(true)
}

pub fn handle_set_code<TimerT: Clone + TimeSinceEpoch>(
    data: &mut ProviderData<TimerT>,
    address: Address,
    code: Bytes,
) -> Result<bool, ProviderError> {
    data.set_code(address, code)?;

    Ok(true)
}

pub fn handle_set_nonce<TimerT: Clone + TimeSinceEpoch>(
    data: &mut ProviderData<TimerT>,
    address: Address,
    nonce: u64,
) -> Result<bool, ProviderError> {
    data.set_nonce(address, nonce)?;

    Ok(true)
}

pub fn handle_set_storage_at<TimerT: Clone + TimeSinceEpoch>(
    data: &mut ProviderData<TimerT>,
    address: Address,
    index: U256,
    value: U256,
) -> Result<bool, ProviderError> {
    data.set_account_storage_slot(address, index, value)?;

    Ok(true)
}
