use edr_eth::{Address, BlockSpec, U256, U64};

use crate::{
    data::ProviderData, requests::validation::validate_post_merge_block_tags, time::TimeSinceEpoch,
    ProviderError,
};

pub fn handle_block_number_request<TimerT: Clone + TimeSinceEpoch>(
    data: &ProviderData<TimerT>,
) -> Result<U64, ProviderError> {
    Ok(U64::from(data.last_block_number()))
}

pub fn handle_chain_id_request<TimerT: Clone + TimeSinceEpoch>(
    data: &ProviderData<TimerT>,
) -> Result<U64, ProviderError> {
    Ok(U64::from(data.chain_id()))
}

pub fn handle_get_transaction_count_request<TimerT: Clone + TimeSinceEpoch>(
    data: &mut ProviderData<TimerT>,
    address: Address,
    block_spec: Option<BlockSpec>,
) -> Result<U256, ProviderError> {
    if let Some(block_spec) = block_spec.as_ref() {
        validate_post_merge_block_tags(data.spec_id(), block_spec)?;
    }

    data.get_transaction_count(address, block_spec.as_ref())
        .map(U256::from)
}
