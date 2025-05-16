use std::{num::NonZeroU64, path::PathBuf, time::SystemTime};

use chrono::{DateTime, Utc};
use edr_eth::{Address, B256, ChainId, HashMap, block::BlobGas};
use edr_evm::hardfork::ChainConfig;
use edr_provider::{AccountOverride, ForkConfig, MiningConfig};
use serde::{Deserialize, Serialize};

use crate::SerializableSecretKey;

/// Helper struct to convert an old scenario format to the new one.
#[derive(Clone, Debug, Serialize, Deserialize)]
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

// /// Placeholder for the actual old scenario provider config.
// /// This should be replaced with the actual old scenario provider config
// pub type ScenarioProviderConfig = super::ScenarioProviderConfig;

/// Custom configuration for the provider that supports serde as we don't want a
/// serde implementation for secret keys.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScenarioProviderConfig {
    pub allow_blocks_with_same_timestamp: bool,
    pub allow_unlimited_contract_size: bool,
    /// Whether to return an `Err` when `eth_call` fails
    pub bail_on_call_failure: bool,
    /// Whether to return an `Err` when a `eth_sendTransaction` fails
    pub bail_on_transaction_failure: bool,
    pub block_gas_limit: NonZeroU64,
    pub chain_id: ChainId,
    pub chain_overrides: HashMap<ChainId, ChainConfig<String>>,
    pub coinbase: Address,
    pub fork: Option<ForkConfig>,
    pub genesis_state: HashMap<Address, AccountOverride>,
    pub hardfork: String,
    #[serde(with = "alloy_serde::quantity::opt")]
    pub initial_base_fee_per_gas: Option<u128>,
    pub initial_blob_gas: Option<BlobGas>,
    /// The initial date of the blockchain, in ISO 8601 format.
    pub initial_date: Option<DateTime<Utc>>,
    pub initial_parent_beacon_block_root: Option<B256>,
    #[serde(with = "alloy_serde::quantity")]
    pub min_gas_price: u128,
    pub mining: MiningConfig,
    pub network_id: u64,
    pub owned_accounts: Vec<SerializableSecretKey>,
}
