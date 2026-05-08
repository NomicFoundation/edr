use core::num::NonZeroU64;
use std::str::FromStr;

use edr_chain_config::{ChainOverride, HardforkActivation, HardforkActivations};
use edr_chain_spec::EvmSpecId;
use edr_eip1559::{BaseFeeActivation, BaseFeeParams, ConstantBaseFeeParams, DynamicBaseFeeParams};
use edr_eip7825::transaction_gas_cap_for_hardfork;
use edr_precompile::PrecompileFn;
use edr_primitives::{Address, ChainId, HashMap, UnknownHardfork, B256};
use edr_provider::{
    config::{ForkConfig, MiningConfig, NetworkConfig},
    observability::ObservabilityConfig,
    AccountOverride,
};
use edr_signer::SecretKey;

/// Configuration option.
#[derive(Clone, Debug, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub enum ConfigOption<T> {
    /// A custom configuration value.
    Custom(T),
    /// Use the default value for this configuration option.
    Default,
    /// Disable the configured option.
    Disable,
}

/// Chain-agnostic configuration for a provider.
#[derive(Clone, Debug)]
pub struct Config {
    pub allow_blocks_with_same_timestamp: bool,
    pub allow_unlimited_contract_size: bool,
    /// Whether to return an `Err` when `eth_call` fails
    pub bail_on_call_failure: bool,
    /// Whether to return an `Err` when a `eth_sendTransaction` fails
    pub bail_on_transaction_failure: bool,
    pub base_fee_params: Option<Vec<(BaseFeeActivation<String>, ConstantBaseFeeParams)>>,
    pub chain_id: ChainId,
    pub coinbase: Address,
    /// The default transaction gas limit to use for RPC call and transaction
    /// requests that do not specify a `gas` value.
    pub default_transaction_gas_limit: NonZeroU64,
    pub genesis_state: HashMap<Address, AccountOverride>,
    pub hardfork: String,
    pub initial_base_fee_per_gas: Option<u128>,
    pub initial_parent_beacon_block_root: Option<B256>,
    pub min_gas_price: u128,
    pub mining: MiningConfig,
    pub network: NetworkConfig<String>,
    pub network_id: u64,
    pub observability: ObservabilityConfig,
    /// Secret keys of owned accounts.
    pub owned_accounts: Vec<SecretKey>,
    pub precompile_overrides: HashMap<Address, PrecompileFn>,
    /// Transaction gas cap, introduced in [EIP-7825].
    ///
    /// When not set, enforcement of the transaction gas cap is disabled and
    /// transactions with any `gas` value are accepted by the mempool and
    /// executed without REVM's transaction gas cap check.
    ///
    /// [EIP-7825]: https://eips.ethereum.org/EIPS/eip-7825
    pub transaction_gas_cap: ConfigOption<u64>,
}

fn parse_hardfork<HardforkT>(hardfork: String) -> napi::Result<HardforkT>
where
    HardforkT: FromStr<Err = UnknownHardfork> + Default + Into<EvmSpecId>,
{
    hardfork.parse().map_err(|UnknownHardfork| {
        napi::Error::new(
            napi::Status::InvalidArg,
            format!("Unknown hardfork: {hardfork}"),
        )
    })
}

impl<HardforkT> TryFrom<Config> for edr_provider::config::Provider<HardforkT>
where
    HardforkT: FromStr<Err = UnknownHardfork>
        + Clone
        + Default
        + Into<edr_chain_spec::EvmSpecId>
        + PartialOrd,
{
    type Error = napi::Error;

    fn try_from(value: Config) -> Result<Self, Self::Error> {
        let base_fee_params: Option<BaseFeeParams<HardforkT>> = value
            .base_fee_params
            .map(|config| {
                config.into_iter().map(|(key, value)| {
                let new_key = match key {
                    BaseFeeActivation::Hardfork(hardfork_str) => {
                        let hardfork = parse_hardfork(hardfork_str)?;
                        BaseFeeActivation::Hardfork(hardfork)
                    }
                    BaseFeeActivation::BlockNumber(number) => {
                        BaseFeeActivation::BlockNumber(number)
                    }
                };
                Ok((new_key, value))
            })
            .collect::<napi::Result<
                Vec<(BaseFeeActivation<HardforkT>, ConstantBaseFeeParams)>
            >>()
            })
            .transpose()?
            .map(|activation| BaseFeeParams::Dynamic(DynamicBaseFeeParams::new(activation)));

        let network = match value.network {
            NetworkConfig::Fork(fork_config) => {
                let chain_overrides = fork_config
                    .chain_overrides
                    .into_iter()
                    .map(|(chain_id, chain_config)| {
                        let hardfork_activation_overrides = chain_config
                            .hardfork_activation_overrides
                            .map(|overrides| {
                                overrides
                                    .into_inner()
                                    .into_iter()
                                    .map(
                                        |HardforkActivation {
                                             condition,
                                             hardfork,
                                         }| {
                                            let hardfork = parse_hardfork(hardfork)?;

                                            Ok(HardforkActivation {
                                                condition,
                                                hardfork,
                                            })
                                        },
                                    )
                                    .collect::<napi::Result<_>>()
                                    .map(HardforkActivations::new)
                            })
                            .transpose()?;

                        let chain_config = ChainOverride {
                            name: chain_config.name,
                            hardfork_activation_overrides,
                        };
                        Ok((chain_id, chain_config))
                    })
                    .collect::<napi::Result<_>>()?;

                ForkConfig {
                    block_number: fork_config.block_number,
                    cache_dir: fork_config.cache_dir,
                    chain_overrides,
                    http_headers: fork_config.http_headers,
                    url: fork_config.url,
                }
                .into()
            }
            NetworkConfig::Local(local_config) => local_config.into(),
        };

        let hardfork = parse_hardfork::<HardforkT>(value.hardfork)?;
        let transaction_gas_cap = match value.transaction_gas_cap {
            ConfigOption::Custom(transaction_gas_cap) => Some(transaction_gas_cap),
            ConfigOption::Default => transaction_gas_cap_for_hardfork(hardfork.clone()),
            ConfigOption::Disable => None,
        };

        Ok(Self {
            allow_blocks_with_same_timestamp: value.allow_blocks_with_same_timestamp,
            allow_unlimited_contract_size: value.allow_unlimited_contract_size,
            bail_on_call_failure: value.bail_on_call_failure,
            bail_on_transaction_failure: value.bail_on_transaction_failure,
            base_fee_params,
            default_transaction_gas_limit: value.default_transaction_gas_limit,
            chain_id: value.chain_id,
            coinbase: value.coinbase,
            genesis_state: value.genesis_state,
            hardfork,
            initial_base_fee_per_gas: value.initial_base_fee_per_gas,
            initial_parent_beacon_block_root: value.initial_parent_beacon_block_root,
            min_gas_price: value.min_gas_price,
            mining: value.mining,
            network,
            network_id: value.network_id,
            observability: value.observability,
            owned_accounts: value.owned_accounts,
            precompile_overrides: value.precompile_overrides,
            transaction_gas_cap,
        })
    }
}
