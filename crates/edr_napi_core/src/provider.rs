use core::num::NonZeroU64;
use std::{path::PathBuf, time::SystemTime};

use edr_eth::{block::BlobGas, AccountInfo, Address, ChainId, HashMap, B256, U256};
use edr_provider::{
    hardfork::{Activations, ForkCondition},
    hardhat_rpc_types::ForkConfig,
    spec::ChainSpec,
    AccountConfig, MiningConfig,
};
use serde::{Deserialize, Serialize};

/// Chain-agnostic configuration for a hardfork activation.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct HardforkActivation {
    pub block_number: u64,
    pub hardfork: String,
}

/// Chain-agnostic configuration for a provider.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Config {
    pub allow_blocks_with_same_timestamp: bool,
    pub allow_unlimited_contract_size: bool,
    pub accounts: Vec<AccountConfig>,
    /// Whether to return an `Err` when `eth_call` fails
    pub bail_on_call_failure: bool,
    /// Whether to return an `Err` when a `eth_sendTransaction` fails
    pub bail_on_transaction_failure: bool,
    pub block_gas_limit: NonZeroU64,
    pub cache_dir: Option<String>,
    pub chain_id: ChainId,
    pub chains: HashMap<ChainId, Vec<HardforkActivation>>,
    pub coinbase: Address,
    #[serde(default)]
    pub enable_rip_7212: bool,
    pub fork: Option<ForkConfig>,
    // Genesis accounts in addition to accounts. Useful for adding impersonated accounts for tests.
    pub genesis_accounts: HashMap<Address, AccountInfo>,
    pub hardfork: String,
    pub initial_base_fee_per_gas: Option<U256>,
    pub initial_blob_gas: Option<BlobGas>,
    pub initial_date: Option<SystemTime>,
    pub initial_parent_beacon_block_root: Option<B256>,
    pub min_gas_price: U256,
    pub mining: MiningConfig,
    pub network_id: u64,
}

impl<ChainSpecT> From<Config> for edr_provider::ProviderConfig<ChainSpecT>
where
    ChainSpecT: ChainSpec<Hardfork: for<'s> From<&'s str>>,
{
    fn from(value: Config) -> Self {
        let cache_dir = PathBuf::from(
            value
                .cache_dir
                .unwrap_or(String::from(edr_defaults::CACHE_DIR)),
        );

        let chains = value
            .chains
            .into_iter()
            .map(|(chain_id, activations)| {
                let activations = activations
                    .into_iter()
                    .map(
                        |HardforkActivation {
                             block_number,
                             hardfork,
                         }| {
                            let condition = ForkCondition::Block(block_number);
                            let hardfork = ChainSpecT::Hardfork::from(&hardfork);

                            (condition, hardfork)
                        },
                    )
                    .collect();

                (chain_id, Activations::new(activations))
            })
            .collect();

        let hardfork = ChainSpecT::Hardfork::from(&value.hardfork);

        Self {
            allow_blocks_with_same_timestamp: value.allow_blocks_with_same_timestamp,
            allow_unlimited_contract_size: value.allow_unlimited_contract_size,
            accounts: value.accounts,
            bail_on_call_failure: value.bail_on_call_failure,
            bail_on_transaction_failure: value.bail_on_transaction_failure,
            block_gas_limit: value.block_gas_limit,
            cache_dir,
            chain_id: value.chain_id,
            chains,
            coinbase: value.coinbase,
            enable_rip_7212: value.enable_rip_7212,
            fork: value.fork,
            genesis_accounts: value.genesis_accounts,
            hardfork,
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
