use core::num::NonZeroU64;
use std::{str::FromStr, time::SystemTime};

use edr_eth::{
    Address, B256, ChainId, HashMap,
    block::BlobGas,
    l1::{self, hardfork::UnknownHardfork},
    signature::SecretKey,
};
use edr_evm::{
    hardfork::{self, ChainConfig},
    precompile::PrecompileFn,
};
use edr_provider::{AccountOverride, config, hardhat_rpc_types::ForkConfig};

/// Chain-agnostic configuration for a provider.
#[derive(Clone, Debug)]
pub struct Config {
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
    pub initial_base_fee_per_gas: Option<u128>,
    pub initial_blob_gas: Option<BlobGas>,
    pub initial_date: Option<SystemTime>,
    pub initial_parent_beacon_block_root: Option<B256>,
    pub min_gas_price: u128,
    pub mining: config::Mining,
    pub network_id: u64,
    pub observability: edr_provider::observability::Config,
    /// Secret keys of owned accounts.
    pub owned_accounts: Vec<SecretKey>,
    pub precompile_overrides: HashMap<Address, PrecompileFn>,
}

impl<HardforkT> TryFrom<Config> for edr_provider::ProviderConfig<HardforkT>
where
    HardforkT: FromStr<Err = UnknownHardfork> + Default + Into<l1::SpecId>,
{
    type Error = napi::Error;

    fn try_from(value: Config) -> Result<Self, Self::Error> {
        let chains = value
            .chain_overrides
            .into_iter()
            .map(|(chain_id, chain_config)| {
                let hardfork_activations = chain_config
                    .hardfork_activations
                    .into_inner()
                    .into_iter()
                    .map(
                        |hardfork::Activation {
                             condition,
                             hardfork,
                         }| {
                            let hardfork = hardfork.parse().map_err(|UnknownHardfork| {
                                napi::Error::new(
                                    napi::Status::InvalidArg,
                                    format!("Unknown hardfork: {hardfork}"),
                                )
                            })?;

                            Ok(hardfork::Activation {
                                condition,
                                hardfork,
                            })
                        },
                    )
                    .collect::<napi::Result<_>>()
                    .map(hardfork::Activations::new)?;

                let chain_config = ChainConfig {
                    name: chain_config.name,
                    hardfork_activations,
                };

                Ok((chain_id, chain_config))
            })
            .collect::<napi::Result<_>>()?;

        let hardfork = value.hardfork.parse().map_err(|UnknownHardfork| {
            napi::Error::new(
                napi::Status::InvalidArg,
                format!("Unknown hardfork: {}", value.hardfork),
            )
        })?;

        Ok(Self {
            allow_blocks_with_same_timestamp: value.allow_blocks_with_same_timestamp,
            allow_unlimited_contract_size: value.allow_unlimited_contract_size,
            bail_on_call_failure: value.bail_on_call_failure,
            bail_on_transaction_failure: value.bail_on_transaction_failure,
            block_gas_limit: value.block_gas_limit,
            chain_id: value.chain_id,
            chains,
            coinbase: value.coinbase,
            fork: value.fork,
            genesis_state: value.genesis_state,
            hardfork,
            initial_base_fee_per_gas: value.initial_base_fee_per_gas,
            initial_blob_gas: value.initial_blob_gas,
            initial_date: value.initial_date,
            initial_parent_beacon_block_root: value.initial_parent_beacon_block_root,
            min_gas_price: value.min_gas_price,
            mining: value.mining,
            network_id: value.network_id,
            observability: value.observability,
            owned_accounts: value.owned_accounts,
            precompile_overrides: value.precompile_overrides,
        })
    }
}
