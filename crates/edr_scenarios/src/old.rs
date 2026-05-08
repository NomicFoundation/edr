use std::num::NonZeroU64;

use edr_chain_config::ChainOverride;
use edr_napi_core::provider::ConfigOption;
use edr_primitives::{Address, ChainId, HashMap, B256};
use edr_provider::{config::MiningConfig, AccountOverride};

use crate::{NetworkConfig, SerializableSecretKey};

/// Helper struct to convert an old scenario format to the new one.
#[derive(Clone, Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScenarioConfig {
    chain_type: Option<String>,
    logger_enabled: bool,
    provider_config: ScenarioProviderConfig,
}

impl From<ScenarioConfig> for super::ScenarioConfig {
    fn from(value: ScenarioConfig) -> Self {
        Self {
            chain_type: value.chain_type,
            logger_enabled: value.logger_enabled,
            provider_config: value.provider_config.into(),
        }
    }
}

/// Placeholder for the actual old scenario provider config.
/// This should be replaced with the actual old scenario provider config
// pub type ScenarioProviderConfig = super::ScenarioProviderConfig;

/// Custom configuration for the provider that supports serde as we don't want a
/// serde implementation for secret keys.
#[derive(Clone, Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScenarioProviderConfig {
    pub allow_blocks_with_same_timestamp: bool,
    pub allow_unlimited_contract_size: bool,
    /// Whether to return an `Err` when `eth_call` fails
    pub bail_on_call_failure: bool,
    /// Whether to return an `Err` when a `eth_sendTransaction` fails
    pub bail_on_transaction_failure: bool,
    pub chain_id: ChainId,
    pub chain_overrides: HashMap<ChainId, ChainOverride<String>>,
    pub coinbase: Address,
    /// The default transaction gas limit to use for RPC call and transaction
    /// requests that do not specify a `gas` value.
    pub default_transaction_gas_limit: NonZeroU64,
    pub genesis_state: HashMap<Address, AccountOverride>,
    pub hardfork: String,
    #[serde(with = "alloy_serde::quantity::opt")]
    pub initial_base_fee_per_gas: Option<u128>,
    pub initial_parent_beacon_block_root: Option<B256>,
    #[serde(with = "alloy_serde::quantity")]
    pub min_gas_price: u128,
    pub mining: MiningConfig,
    pub network: NetworkConfig<String>,
    pub network_id: u64,
    pub owned_accounts: Vec<SerializableSecretKey>,
    /// Transaction gas cap, introduced in [EIP-7825].
    ///
    /// When not set, enforcement of the transaction gas cap is disabled and
    /// transactions with any `gas` value are accepted by the mempool and
    /// executed without REVM's transaction gas cap check.
    ///
    /// [EIP-7825]: https://eips.ethereum.org/EIPS/eip-7825
    #[serde(default)]
    pub transaction_gas_cap: Option<u64>,
}

impl From<ScenarioProviderConfig> for super::ScenarioProviderConfig {
    fn from(value: ScenarioProviderConfig) -> Self {
        Self {
            allow_blocks_with_same_timestamp: value.allow_blocks_with_same_timestamp,
            allow_unlimited_contract_size: value.allow_unlimited_contract_size,
            bail_on_call_failure: value.bail_on_call_failure,
            bail_on_transaction_failure: value.bail_on_transaction_failure,
            chain_id: value.chain_id,
            chain_overrides: value.chain_overrides,
            coinbase: value.coinbase,
            default_transaction_gas_limit: value.default_transaction_gas_limit,
            genesis_state: value.genesis_state,
            hardfork: value.hardfork,
            initial_base_fee_per_gas: value.initial_base_fee_per_gas,
            initial_parent_beacon_block_root: value.initial_parent_beacon_block_root,
            min_gas_price: value.min_gas_price,
            mining: value.mining,
            network: value.network,
            network_id: value.network_id,
            owned_accounts: value.owned_accounts,
            transaction_gas_cap: if let Some(value) = value.transaction_gas_cap {
                ConfigOption::Custom(value)
            } else {
                ConfigOption::Default
            },
        }
    }
}
