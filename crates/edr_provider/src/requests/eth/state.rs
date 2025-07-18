use edr_eth::{
    l1, transaction::TransactionValidation, utils::u256_to_padded_hex, Address, BlockSpec, Bytes,
    U256,
};

use crate::{
    data::ProviderData, requests::validation::validate_post_merge_block_tags,
    spec::SyncProviderSpec, time::TimeSinceEpoch, ProviderErrorForChainSpec,
};

pub fn handle_get_balance_request<
    ChainSpecT: SyncProviderSpec<
        TimerT,
        BlockEnv: Default,
        SignedTransaction: Default
                               + TransactionValidation<
            ValidationError: From<l1::InvalidTransaction> + PartialEq,
        >,
    >,
    TimerT: Clone + TimeSinceEpoch,
>(
    data: &mut ProviderData<ChainSpecT, TimerT>,
    address: Address,
    block_spec: Option<BlockSpec>,
) -> Result<U256, ProviderErrorForChainSpec<ChainSpecT>> {
    if let Some(block_spec) = block_spec.as_ref() {
        validate_post_merge_block_tags::<ChainSpecT>(data.hardfork(), block_spec)?;
    }

    data.balance(address, block_spec.as_ref())
}

pub fn handle_get_code_request<
    ChainSpecT: SyncProviderSpec<
        TimerT,
        BlockEnv: Default,
        SignedTransaction: Default
                               + TransactionValidation<
            ValidationError: From<l1::InvalidTransaction> + PartialEq,
        >,
    >,
    TimerT: Clone + TimeSinceEpoch,
>(
    data: &mut ProviderData<ChainSpecT, TimerT>,
    address: Address,
    block_spec: Option<BlockSpec>,
) -> Result<Bytes, ProviderErrorForChainSpec<ChainSpecT>> {
    if let Some(block_spec) = block_spec.as_ref() {
        validate_post_merge_block_tags::<ChainSpecT>(data.hardfork(), block_spec)?;
    }

    data.get_code(address, block_spec.as_ref())
}

pub fn handle_get_storage_at_request<
    ChainSpecT: SyncProviderSpec<
        TimerT,
        BlockEnv: Default,
        SignedTransaction: Default
                               + TransactionValidation<
            ValidationError: From<l1::InvalidTransaction> + PartialEq,
        >,
    >,
    TimerT: Clone + TimeSinceEpoch,
>(
    data: &mut ProviderData<ChainSpecT, TimerT>,
    address: Address,
    index: U256,
    block_spec: Option<BlockSpec>,
) -> Result<String, ProviderErrorForChainSpec<ChainSpecT>> {
    if let Some(block_spec) = block_spec.as_ref() {
        validate_post_merge_block_tags::<ChainSpecT>(data.hardfork(), block_spec)?;
    }

    let storage = data.get_storage_at(address, index, block_spec.as_ref())?;
    Ok(u256_to_padded_hex(&storage))
}
