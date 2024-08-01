use core::fmt::Debug;

use edr_eth::{
    result::InvalidTransaction, transaction::TransactionValidation, Address, BlockSpec, U256, U64,
};
use edr_evm::chain_spec::{ChainSpec, SyncChainSpec};

use crate::{
    data::ProviderData, requests::validation::validate_post_merge_block_tags, time::TimeSinceEpoch,
    ProviderError,
};

pub fn handle_block_number_request<
    ChainSpecT: SyncChainSpec<Hardfork: Debug>,
    LoggerErrorT: Debug,
    TimerT: Clone + TimeSinceEpoch,
>(
    data: &ProviderData<ChainSpecT, LoggerErrorT, TimerT>,
) -> Result<U64, ProviderError<ChainSpecT, LoggerErrorT>> {
    Ok(U64::from(data.last_block_number()))
}

pub fn handle_chain_id_request<
    ChainSpecT: SyncChainSpec<Hardfork: Debug>,
    LoggerErrorT: Debug,
    TimerT: Clone + TimeSinceEpoch,
>(
    data: &ProviderData<ChainSpecT, LoggerErrorT, TimerT>,
) -> Result<U64, ProviderError<ChainSpecT, LoggerErrorT>> {
    Ok(U64::from(data.chain_id()))
}

pub fn handle_get_transaction_count_request<
    ChainSpecT: SyncChainSpec<
        Block: Default,
        Hardfork: Debug,
        Transaction: Default
                         + TransactionValidation<
            ValidationError: From<InvalidTransaction> + PartialEq,
        >,
    >,
    LoggerErrorT: Debug,
    TimerT: Clone + TimeSinceEpoch,
>(
    data: &mut ProviderData<ChainSpecT, LoggerErrorT, TimerT>,
    address: Address,
    block_spec: Option<BlockSpec>,
) -> Result<U256, ProviderError<ChainSpecT, LoggerErrorT>> {
    if let Some(block_spec) = block_spec.as_ref() {
        validate_post_merge_block_tags(data.evm_spec_id(), block_spec)?;
    }

    data.get_transaction_count(address, block_spec.as_ref())
        .map(U256::from)
}
