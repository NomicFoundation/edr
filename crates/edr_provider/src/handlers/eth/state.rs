use edr_eth::BlockSpec;
use edr_primitives::{Address, U256};
use edr_rpc_types::RpcAddress;

use crate::{
    handlers::error::DynProviderError, requests::validation::validate_post_merge_block_tags,
    time::TimeSinceEpoch, ProviderData, SyncProviderSpec,
};

/// Parameters for the `eth_getTransactionCount` JSON-RPC method.
#[derive(serde::Deserialize)]
#[serde(from = "(RpcAddress, Option<BlockSpec>)")]
pub struct EthGetTransactionCountParams {
    /// `DATA, 20 bytes` - Address to check the transaction count for.
    pub address: Address,
    /// `BlockSpec` - Block number, tag, or EIP-1898 block identifier.
    /// Defaults to `"latest"`.
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

/// Handler for `eth_getTransactionCount`.
///
/// Returns the nonce of the account corresponding to the provided address.
///
/// NOTE: This method is named `eth_getTransactionCount` for historical
/// reasons, as up until the pectra hardfork, the nonce was equivalent to
/// the number of transactions sent from the address. This changed due to
/// the inclusion of EIP-7702.
///
/// ## Result
///
/// `QUANTITY` - The nonce of the account at the provided address.
///
/// ## Example
///
/// **Request:**
///
/// ```json
/// {
///   "params": ["0x0000000000000000000000000000000000000001", "latest"]
/// }
/// ```
///
/// **Response:**
///
/// ```json
/// "0x1"
/// ```
///
/// ## Implementation details
///
/// - Post-merge block tags (`"safe"`, `"finalized"`) are only available for the
///   merge hardfork and later.
pub fn handle_get_transaction_count_request<
    ChainSpecT: SyncProviderSpec<TimerT>,
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
