use std::{num::NonZeroU64, time::SystemTime};

use edr_eth::{
    Address, B256, ChainId, HashMap, KECCAK_EMPTY, U256, account::AccountInfo, block::BlobGas,
    signature::public_key_to_address,
};
use edr_evm::hardfork::ChainConfig;
use edr_provider::{AccountConfig, MiningConfig};
use serde::{Deserialize, Serialize};

/// Configuration for forking a blockchain
#[derive(Clone, Debug, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ForkConfig {
    pub json_rpc_url: String,
    pub block_number: Option<u64>,
    pub http_headers: Option<std::collections::HashMap<String, String>>,
}

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
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScenarioProviderConfig {
    pub allow_blocks_with_same_timestamp: bool,
    pub allow_unlimited_contract_size: bool,
    pub accounts: Vec<ScenarioOwnedAccount>,
    /// Whether to return an `Err` when `eth_call` fails
    pub bail_on_call_failure: bool,
    /// Whether to return an `Err` when a `eth_sendTransaction` fails
    pub bail_on_transaction_failure: bool,
    pub block_gas_limit: NonZeroU64,
    pub cache_dir: Option<String>,
    pub chain_id: ChainId,
    pub chains: HashMap<ChainId, ChainConfig<String>>,
    pub coinbase: Address,
    #[serde(default)]
    pub enable_rip_7212: bool,
    pub fork: Option<ForkConfig>,
    pub genesis_state: HashMap<Address, AccountConfig>,
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
}

impl From<ScenarioProviderConfig> for super::ScenarioProviderConfig {
    fn from(value: ScenarioProviderConfig) -> Self {
        let mut genesis_state = value.genesis_state;

        let owned_accounts = value
            .accounts
            .into_iter()
            .map(
                |ScenarioOwnedAccount {
                     secret_key,
                     balance,
                 }| {
                    let address = public_key_to_address(secret_key.public_key());

                    genesis_state
                        .entry(address)
                        .and_modify(|account| account.info.balance = balance)
                        .or_insert(AccountConfig {
                            info: AccountInfo {
                                balance,
                                nonce: 0,
                                code: None,
                                code_hash: KECCAK_EMPTY,
                            },
                            storage: HashMap::new(),
                        });

                    secret_key.into()
                },
            )
            .collect();

        Self {
            allow_blocks_with_same_timestamp: value.allow_blocks_with_same_timestamp,
            allow_unlimited_contract_size: value.allow_unlimited_contract_size,
            bail_on_call_failure: value.bail_on_call_failure,
            bail_on_transaction_failure: value.bail_on_transaction_failure,
            block_gas_limit: value.block_gas_limit,
            chain_id: value.chain_id,
            chains: value.chains,
            coinbase: value.coinbase,
            fork: value.fork.map(|fork_config| super::ForkConfig {
                block_number: fork_config.block_number,
                // We don't support custom cache directories for replaying scenarios, so set it
                // to the default directory.
                cache_dir: edr_defaults::CACHE_DIR.into(),
                http_headers: fork_config.http_headers,
                url: fork_config.json_rpc_url,
            }),
            genesis_state,
            hardfork: value.hardfork,
            initial_base_fee_per_gas: value.initial_base_fee_per_gas,
            initial_blob_gas: value.initial_blob_gas,
            initial_date: value.initial_date,
            initial_parent_beacon_block_root: value.initial_parent_beacon_block_root,
            min_gas_price: value.min_gas_price,
            mining: value.mining,
            network_id: value.network_id,
            owned_accounts,
        }
    }
}

/// Configuration input for a single account
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScenarioOwnedAccount {
    /// the secret key of the account
    #[serde(with = "secret_key_serde")]
    pub secret_key: k256::SecretKey,
    /// the balance of the account
    pub balance: U256,
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
