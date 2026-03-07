use edr_chain_spec::TransactionValidation;
use edr_eth::BlockSpec;
use edr_primitives::{Address, U256};
use edr_rpc_types::RpcAddress;

use crate::{
    handlers::error::DynProviderError, requests::validation::validate_post_merge_block_tags,
    time::TimeSinceEpoch, ProviderData, ProviderErrorForChainSpec, SyncProviderSpec,
};

#[derive(serde::Deserialize)]
#[serde(from = "(RpcAddress, Option<BlockSpec>)")]
pub struct EthGetTransactionCountParams {
    pub address: Address,
    pub block_spec: Option<BlockSpec>,
}

impl From<(RpcAddress, Option<BlockSpec>)> for EthGetTransactionCountParams {
    fn from(value: (RpcAddress, Option<BlockSpec>)) -> Self {
        Self {
            address: value.0.into(),
            block_spec: value.1,
        }
    }
}

/// Method name for the `eth_getTransactionCount` JSON-RPC method.
pub const ETH_GET_TRANSACTION_COUNT_METHOD: &str = "eth_getTransactionCount";

pub fn handle_get_transaction_count_request<
    ChainSpecT: SyncProviderSpec<
        TimerT,
        SignedTransaction: Default + TransactionValidation<ValidationError: PartialEq>,
    >,
    TimerT: Clone + TimeSinceEpoch,
>(
    data: &mut ProviderData<ChainSpecT, TimerT>,
    params: EthGetTransactionCountParams,
) -> Result<U256, DynProviderError> {
    if let Some(block_spec) = params.block_spec.as_ref() {
        validate_post_merge_block_tags::<ChainSpecT, TimerT>(data.hardfork(), block_spec)
            .map_err(DynProviderError::new)?;
    }

    data.get_transaction_count(params.address, params.block_spec.as_ref())
        .map_or_else(
            |error| Err(DynProviderError::new(error)),
            |nonce| Ok(U256::from(nonce)),
        )
}
