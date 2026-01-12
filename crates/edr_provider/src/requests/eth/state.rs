use alloy_rpc_types::EIP1186AccountProofResponse;
use edr_chain_spec::TransactionValidation;
use edr_eth::BlockSpec;
use edr_primitives::{Address, Bytes, B256, U256};

use crate::{
    data::ProviderData, requests::validation::validate_post_merge_block_tags,
    spec::SyncProviderSpec, time::TimeSinceEpoch, utils::u256_to_padded_hex,
    ProviderErrorForChainSpec,
};

pub fn handle_get_balance_request<
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

    data.balance(address, block_spec.as_ref())
}

pub fn handle_get_code_request<
    ChainSpecT: SyncProviderSpec<
        TimerT,
        SignedTransaction: Default + TransactionValidation<ValidationError: PartialEq>,
    >,
    TimerT: Clone + TimeSinceEpoch,
>(
    data: &mut ProviderData<ChainSpecT, TimerT>,
    address: Address,
    block_spec: Option<BlockSpec>,
) -> Result<Bytes, ProviderErrorForChainSpec<ChainSpecT>> {
    if let Some(block_spec) = block_spec.as_ref() {
        validate_post_merge_block_tags::<ChainSpecT, TimerT>(data.hardfork(), block_spec)?;
    }

    data.get_code(address, block_spec.as_ref())
}

pub fn handle_get_proof_request<
    ChainSpecT: SyncProviderSpec<
        TimerT,
        SignedTransaction: Default + TransactionValidation<ValidationError: PartialEq>,
    >,
    TimerT: Clone + TimeSinceEpoch,
>(
    data: &mut ProviderData<ChainSpecT, TimerT>,
    address: Address,
    storage_keys: Vec<B256>,
    block_spec: BlockSpec,
) -> Result<EIP1186AccountProofResponse, ProviderErrorForChainSpec<ChainSpecT>> {
    validate_post_merge_block_tags::<ChainSpecT, TimerT>(data.hardfork(), &block_spec)?;
    data.get_proof(address, storage_keys, &block_spec)
}

pub fn handle_get_storage_at_request<
    ChainSpecT: SyncProviderSpec<
        TimerT,
        SignedTransaction: Default + TransactionValidation<ValidationError: PartialEq>,
    >,
    TimerT: Clone + TimeSinceEpoch,
>(
    data: &mut ProviderData<ChainSpecT, TimerT>,
    address: Address,
    index: U256,
    block_spec: Option<BlockSpec>,
) -> Result<String, ProviderErrorForChainSpec<ChainSpecT>> {
    if let Some(block_spec) = block_spec.as_ref() {
        validate_post_merge_block_tags::<ChainSpecT, TimerT>(data.hardfork(), block_spec)?;
    }

    let storage = data.get_storage_at(address, index, block_spec.as_ref())?;
    Ok(u256_to_padded_hex(&storage))
}
