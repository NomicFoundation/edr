use core::fmt::Debug;

use alloy_dyn_abi::eip712::TypedData;
use edr_eth::{Address, Bytes};

use crate::{data::ProviderData, time::TimeSinceEpoch, ProviderError};

pub fn handle_sign_request<LoggerErrorT: Debug, TimerT: Clone + TimeSinceEpoch>(
    data: &ProviderData<LoggerErrorT, TimerT>,
    message: Bytes,
    address: Address,
) -> Result<Bytes, ProviderError<LoggerErrorT>> {
    Ok((&data.sign(&address, message)?).into())
}

pub fn handle_sign_typed_data_v4<LoggerErrorT: Debug, TimerT: Clone + TimeSinceEpoch>(
    data: &ProviderData<LoggerErrorT, TimerT>,
    address: Address,
    message: TypedData,
) -> Result<Bytes, ProviderError<LoggerErrorT>> {
    Ok((&data.sign_typed_data_v4(&address, &message)?).into())
}
