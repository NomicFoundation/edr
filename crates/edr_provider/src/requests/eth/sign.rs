use alloy_dyn_abi::eip712::TypedData;
use edr_eth::{Address, Bytes};

use crate::{data::ProviderData, time::TimeSinceEpoch, ProviderError};

pub fn handle_sign_request<TimerT: Clone + TimeSinceEpoch>(
    data: &ProviderData<TimerT>,
    message: Bytes,
    address: Address,
) -> Result<Bytes, ProviderError> {
    Ok((&data.sign(&address, message)?).into())
}

pub fn handle_sign_typed_data_v4<TimerT: Clone + TimeSinceEpoch>(
    data: &ProviderData<TimerT>,
    address: Address,
    message: TypedData,
) -> Result<Bytes, ProviderError> {
    Ok((&data.sign_typed_data_v4(&address, &message)?).into())
}
