use std::num::NonZeroU64;

use chrono::{DateTime, Utc};
use edr_block_header::BlobGas;
use edr_chain_config::ChainOverride;
use edr_eip7825::transaction_gas_cap_for_hardfork;
use edr_primitives::{Address, ChainId, HashMap, B256};
use edr_provider::{
    config::{ForkConfig, IntervalConfig, MemPoolConfig},
    AccountOverride,
};
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

/// Placeholder for the actual old scenario provider config.
/// This should be replaced with the actual old scenario provider config
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
    pub chain_overrides: HashMap<ChainId, ChainOverride<String>>,
    pub coinbase: Address,
    pub fork: Option<ForkConfig<String>>,
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
    /// Transaction gas cap, introduced in [EIP-7825].
    ///
    /// When not set, will default to value defined by the used hardfork
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
            default_transaction_gas_limit: if let Some(transaction_gas_cap) =
                value.transaction_gas_cap
            {
                NonZeroU64::new(transaction_gas_cap)
                    .expect("transaction_gas_cap must be greater than 0")
            } else {
                value.block_gas_limit
            },
            chain_id: value.chain_id,
            chain_overrides: value.chain_overrides,
            coinbase: value.coinbase,
            genesis_state: value.genesis_state,
            hardfork: value.hardfork.clone(),
            initial_base_fee_per_gas: value.initial_base_fee_per_gas,
            initial_parent_beacon_block_root: value.initial_parent_beacon_block_root,
            min_gas_price: value.min_gas_price,
            mining: value.mining.convert(value.block_gas_limit),
            network: if let Some(fork_config) = value.fork {
                super::NetworkConfig::Fork(fork_config)
            } else {
                super::NetworkConfig::Local(super::LocalConfig {
                    genesis_blob_gas: value.initial_blob_gas,
                    genesis_block_gas_limit: value.block_gas_limit,
                    genesis_block_time: value.initial_date,
                })
            },
            network_id: value.network_id,
            owned_accounts: value.owned_accounts.into_iter().map(Into::into).collect(),
            transaction_gas_cap: value.transaction_gas_cap.or_else(|| {
                let hardfork: Option<edr_chain_l1::Hardfork> = value.hardfork.parse().ok();

                hardfork.and_then(|hardfork| transaction_gas_cap_for_hardfork(hardfork))
            }),
        }
    }
}

/// Configuration for the provider's miner.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MiningConfig {
    pub auto_mine: bool,
    pub interval: Option<IntervalConfig>,
    pub mem_pool: MemPoolConfig,
}

impl MiningConfig {
    pub fn convert(self, block_gas_limit: NonZeroU64) -> super::MiningConfig {
        super::MiningConfig {
            auto_mine: self.auto_mine,
            block_gas_limit: Some(block_gas_limit),
            interval: self.interval,
            mem_pool: self.mem_pool,
        }
    }
}
