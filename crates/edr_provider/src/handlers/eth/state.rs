use edr_chain_spec::TransactionValidation;
use edr_eth::BlockSpec;
use edr_primitives::Address;

use crate::{time::TimeSinceEpoch, ProviderData, SyncProviderSpec};

fn deserialize_get_transaction_count_params() {}

pub fn handle_get_transaction_count_request<
    ChainSpecT: SyncProviderSpec<
        TimerT,
        SignedTransaction: Default + TransactionValidation<ValidationError: PartialEq>,
    >,
    TimerT: Clone + TimeSinceEpoch,
>(
    data: &mut ProviderData<ChainSpecT, TimerT>,
    address: Address,
    block_spec: Option<BlockSpec>,
) -> Result<U256, ProviderErrorForChainSpec<ChainSpecT>> {
    if let Some(block_spec) = block_spec.as_ref() {
        validate_post_merge_block_tags::<ChainSpecT, TimerT>(data.hardfork(), block_spec)?;
    }

    data.get_transaction_count(address, block_spec.as_ref())
        .map(U256::from)
}
