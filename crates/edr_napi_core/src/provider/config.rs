use core::num::NonZeroU64;
use std::{str::FromStr, time::SystemTime};

use edr_chain_l1::UnknownHardfork;
use edr_eip1559::{BaseFeeActivation, BaseFeeParams, ConstantBaseFeeParams, DynamicBaseFeeParams};
use edr_eth::{block::BlobGas, Address, ChainId, HashMap, B256};
use edr_evm::{
    hardfork::{self, ChainOverride},
    precompile::PrecompileFn,
};
use edr_evm_spec::EvmSpecId;
use edr_provider::{config, AccountOverride, ForkConfig};
use edr_signer::SecretKey;

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
    pub block_gas_limit: NonZeroU64,
    pub chain_id: ChainId,
    pub coinbase: Address,
    pub fork: Option<ForkConfig<String>>,
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

impl<HardforkT> TryFrom<Config> for edr_provider::ProviderConfig<HardforkT>
where
    HardforkT: FromStr<Err = UnknownHardfork>
        + Default
        + Into<edr_evm_spec::EvmSpecId>
        + Clone
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

        let fork = value
            .fork
            .map(|fork| -> napi::Result<ForkConfig<HardforkT>> {
                let chain_overrides = fork
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
                                        |hardfork::Activation {
                                             condition,
                                             hardfork,
                                         }| {
                                            let hardfork = parse_hardfork(hardfork)?;

                                            Ok(hardfork::Activation {
                                                condition,
                                                hardfork,
                                            })
                                        },
                                    )
                                    .collect::<napi::Result<_>>()
                                    .map(hardfork::Activations::new)
                            })
                            .transpose()?;

                        let chain_config = ChainOverride {
                            name: chain_config.name,
                            hardfork_activation_overrides,
                        };
                        Ok((chain_id, chain_config))
                    })
                    .collect::<napi::Result<_>>()?;

                Ok(edr_provider::ForkConfig {
                    block_number: fork.block_number,
                    cache_dir: fork.cache_dir,
                    chain_overrides,
                    http_headers: fork.http_headers,
                    url: fork.url,
                })
            })
            .transpose()?;

        let hardfork = parse_hardfork(value.hardfork)?;

        Ok(Self {
            allow_blocks_with_same_timestamp: value.allow_blocks_with_same_timestamp,
            allow_unlimited_contract_size: value.allow_unlimited_contract_size,
            bail_on_call_failure: value.bail_on_call_failure,
            bail_on_transaction_failure: value.bail_on_transaction_failure,
            base_fee_params,
            block_gas_limit: value.block_gas_limit,
            chain_id: value.chain_id,
            coinbase: value.coinbase,
            fork,
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
