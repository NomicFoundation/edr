/// Types for benchmark scenario collection.
/// We are replicating the provider config here, as we need to be able to
/// serialize secret keys for scenario collecting, but we don't want to include
/// this in the production code to prevent secrets from accidentally leaking
/// into logs.
use std::{num::NonZeroU64, path::PathBuf, time::SystemTime};

use edr_eth::{
    block::BlobGas, spec::HardforkActivations, AccountInfo, Address, HashMap, SpecId, B256, U256,
};
use edr_evm::alloy_primitives::ChainId;
use edr_provider::{hardhat_rpc_types::ForkConfig, MiningConfig};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ScenarioConfig {
    pub provider_config: ScenarioProviderConfig,
    pub logger_enabled: bool,
}

/// Custom configuration for the provider that supports serde as we don't want a
/// serde implementation for secret keys.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScenarioProviderConfig {
    pub allow_blocks_with_same_timestamp: bool,
    pub allow_unlimited_contract_size: bool,
    pub accounts: Vec<ScenarioAccountConfig>,
    /// Whether to return an `Err` when `eth_call` fails
    pub bail_on_call_failure: bool,
    /// Whether to return an `Err` when a `eth_sendTransaction` fails
    pub bail_on_transaction_failure: bool,
    pub block_gas_limit: NonZeroU64,
    pub cache_dir: PathBuf,
    pub chain_id: ChainId,
    pub chains: HashMap<ChainId, HardforkActivations>,
    pub coinbase: Address,
    pub enable_rip_7212: bool,
    pub fork: Option<ForkConfig>,
    // Genesis accounts in addition to accounts. Useful for adding impersonated accounts for tests.
    pub genesis_accounts: HashMap<Address, AccountInfo>,
    pub hardfork: SpecId,
    pub initial_base_fee_per_gas: Option<U256>,
    pub initial_blob_gas: Option<BlobGas>,
    pub initial_date: Option<SystemTime>,
    pub initial_parent_beacon_block_root: Option<B256>,
    pub min_gas_price: U256,
    pub mining: MiningConfig,
    pub network_id: u64,
}

impl From<ScenarioProviderConfig> for edr_provider::ProviderConfig {
    fn from(value: ScenarioProviderConfig) -> Self {
        Self {
            allow_blocks_with_same_timestamp: value.allow_blocks_with_same_timestamp,
            allow_unlimited_contract_size: value.allow_unlimited_contract_size,
            accounts: value
                .accounts
                .into_iter()
                .map(edr_provider::AccountConfig::from)
                .collect::<Vec<_>>(),
            bail_on_call_failure: value.bail_on_call_failure,
            bail_on_transaction_failure: value.bail_on_transaction_failure,
            block_gas_limit: value.block_gas_limit,
            cache_dir: value.cache_dir,
            chain_id: value.chain_id,
            chains: value.chains,
            coinbase: value.coinbase,
            enable_rip_7212: value.enable_rip_7212,
            fork: value.fork,
            genesis_accounts: value.genesis_accounts,
            hardfork: value.hardfork,
            initial_base_fee_per_gas: value.initial_base_fee_per_gas,
            initial_blob_gas: value.initial_blob_gas,
            initial_date: value.initial_date,
            initial_parent_beacon_block_root: value.initial_parent_beacon_block_root,
            min_gas_price: value.min_gas_price,
            mining: value.mining,
            network_id: value.network_id,
        }
    }
}

impl From<edr_provider::ProviderConfig> for ScenarioProviderConfig {
    fn from(value: edr_provider::ProviderConfig) -> Self {
        Self {
            allow_blocks_with_same_timestamp: value.allow_blocks_with_same_timestamp,
            allow_unlimited_contract_size: value.allow_unlimited_contract_size,
            accounts: value
                .accounts
                .into_iter()
                .map(ScenarioAccountConfig::from)
                .collect::<Vec<_>>(),
            bail_on_call_failure: value.bail_on_call_failure,
            bail_on_transaction_failure: value.bail_on_transaction_failure,
            block_gas_limit: value.block_gas_limit,
            cache_dir: value.cache_dir,
            chain_id: value.chain_id,
            chains: value.chains,
            coinbase: value.coinbase,
            enable_rip_7212: value.enable_rip_7212,
            fork: value.fork,
            genesis_accounts: value.genesis_accounts,
            hardfork: value.hardfork,
            initial_base_fee_per_gas: value.initial_base_fee_per_gas,
            initial_blob_gas: value.initial_blob_gas,
            initial_date: value.initial_date,
            initial_parent_beacon_block_root: value.initial_parent_beacon_block_root,
            min_gas_price: value.min_gas_price,
            mining: value.mining,
            network_id: value.network_id,
        }
    }
}

/// Configuration input for a single account
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScenarioAccountConfig {
    /// the secret key of the account
    #[serde(with = "secret_key_serde")]
    pub secret_key: k256::SecretKey,
    /// the balance of the account
    pub balance: U256,
}

impl From<ScenarioAccountConfig> for edr_provider::AccountConfig {
    fn from(value: ScenarioAccountConfig) -> Self {
        Self {
            secret_key: value.secret_key,
            balance: value.balance,
        }
    }
}

impl From<edr_provider::AccountConfig> for ScenarioAccountConfig {
    fn from(value: edr_provider::AccountConfig) -> Self {
        Self {
            secret_key: value.secret_key,
            balance: value.balance,
        }
    }
}

mod secret_key_serde {
    use edr_test_utils::secret_key::{secret_key_from_str, secret_key_to_str};
    use serde::Deserialize;

    pub(super) fn serialize<S>(
        secret_key: &k256::SecretKey,
        serializer: S,
    ) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&secret_key_to_str(secret_key))
    }

    pub(super) fn deserialize<'de, D>(deserializer: D) -> Result<k256::SecretKey, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let s = <&str as Deserialize>::deserialize(deserializer)?;
        secret_key_from_str(s).map_err(serde::de::Error::custom)
    }
}
