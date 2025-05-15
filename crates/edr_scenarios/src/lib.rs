/// Types for benchmark scenario collection.
/// We are replicating the provider config here, as we need to be able to
/// serialize secret keys for scenario collecting, but we don't want to include
/// this in the production code to prevent secrets from accidentally leaking
/// into logs.
pub mod old;

use std::{num::NonZeroU64, path::PathBuf, time::SystemTime};

use edr_eth::{Address, B256, ChainId, HashMap, block::BlobGas};
use edr_evm::hardfork::ChainConfig;
use edr_napi_core::provider::Config as ProviderConfig;
use edr_provider::{AccountOverride, MiningConfig, hardhat_rpc_types::ForkConfig};
use edr_test_utils::secret_key::{secret_key_from_str, secret_key_to_str};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScenarioConfig {
    pub chain_type: Option<String>,
    pub logger_enabled: bool,
    pub provider_config: ScenarioProviderConfig,
}

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
    pub initial_date: Option<SystemTime>,
    pub initial_parent_beacon_block_root: Option<B256>,
    #[serde(with = "alloy_serde::quantity")]
    pub min_gas_price: u128,
    pub mining: MiningConfig,
    pub network_id: u64,
    pub owned_accounts: Vec<SerializableSecretKey>,
}

impl From<ScenarioProviderConfig> for ProviderConfig {
    fn from(value: ScenarioProviderConfig) -> Self {
        // We don't support custom cache directories for replaying scenarios, so set it
        // to the default directory.
        let fork = value.fork.map(|mut fork_config| {
            fork_config.cache_dir = PathBuf::from(edr_defaults::CACHE_DIR);

            fork_config
        });

        Self {
            allow_blocks_with_same_timestamp: value.allow_blocks_with_same_timestamp,
            allow_unlimited_contract_size: value.allow_unlimited_contract_size,
            bail_on_call_failure: value.bail_on_call_failure,
            bail_on_transaction_failure: value.bail_on_transaction_failure,
            block_gas_limit: value.block_gas_limit,
            chain_id: value.chain_id,
            chain_overrides: value.chain_overrides,
            coinbase: value.coinbase,
            fork,
            genesis_state: value.genesis_state,
            hardfork: value.hardfork,
            initial_base_fee_per_gas: value.initial_base_fee_per_gas,
            initial_blob_gas: value.initial_blob_gas,
            initial_date: value.initial_date,
            initial_parent_beacon_block_root: value.initial_parent_beacon_block_root,
            min_gas_price: value.min_gas_price,
            mining: value.mining,
            network_id: value.network_id,
            observability: edr_provider::observability::Config::default(),
            owned_accounts: value
                .owned_accounts
                .into_iter()
                .map(SerializableSecretKey::into_inner)
                .collect::<Vec<_>>(),
            // Overriding precompiles is not supported in scenarios
            precompile_overrides: HashMap::new(),
        }
    }
}

impl TryFrom<ProviderConfig> for ScenarioProviderConfig {
    type Error = anyhow::Error;

    fn try_from(value: ProviderConfig) -> Result<Self, Self::Error> {
        if !value.precompile_overrides.is_empty() {
            return Err(anyhow::anyhow!(
                "Precompile overrides are not supported in scenarios"
            ));
        }

        Ok(Self {
            allow_blocks_with_same_timestamp: value.allow_blocks_with_same_timestamp,
            allow_unlimited_contract_size: value.allow_unlimited_contract_size,
            bail_on_call_failure: value.bail_on_call_failure,
            bail_on_transaction_failure: value.bail_on_transaction_failure,
            block_gas_limit: value.block_gas_limit,
            chain_id: value.chain_id,
            chain_overrides: value.chain_overrides,
            coinbase: value.coinbase,
            fork: value.fork,
            genesis_state: value.genesis_state,
            hardfork: value.hardfork,
            initial_base_fee_per_gas: value.initial_base_fee_per_gas,
            initial_blob_gas: value.initial_blob_gas,
            initial_date: value.initial_date,
            initial_parent_beacon_block_root: value.initial_parent_beacon_block_root,
            min_gas_price: value.min_gas_price,
            mining: value.mining,
            network_id: value.network_id,
            owned_accounts: value
                .owned_accounts
                .into_iter()
                .map(SerializableSecretKey::from)
                .collect(),
        })
    }
}

#[repr(transparent)]
#[derive(Clone, Debug)]
pub struct SerializableSecretKey(k256::SecretKey);

impl SerializableSecretKey {
    fn into_inner(self) -> k256::SecretKey {
        self.0
    }
}

impl From<k256::SecretKey> for SerializableSecretKey {
    fn from(value: k256::SecretKey) -> Self {
        Self(value)
    }
}

impl<'de> serde::Deserialize<'de> for SerializableSecretKey {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let secret_key = <&str as Deserialize>::deserialize(deserializer)?;
        let secret_key = secret_key_from_str(secret_key).map_err(serde::de::Error::custom)?;

        Ok(Self(secret_key))
    }
}

impl serde::Serialize for SerializableSecretKey {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&secret_key_to_str(&self.0))
    }
}
