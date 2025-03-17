use core::num::NonZeroU64;
use std::{path::PathBuf, time::SystemTime};

use edr_eth::{block::BlobGas, l1, Address, ChainId, HashMap, B256};
use edr_provider::{
    config,
    hardfork::{Activations, ForkCondition},
    hardhat_rpc_types::ForkConfig,
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
    pub accounts: Vec<config::OwnedAccount>,
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
    pub genesis_state: HashMap<Address, config::Account>,
    pub hardfork: String,
    #[serde(with = "alloy_serde::quantity::opt")]
    pub initial_base_fee_per_gas: Option<u128>,
    pub initial_blob_gas: Option<BlobGas>,
    pub initial_date: Option<SystemTime>,
    pub initial_parent_beacon_block_root: Option<B256>,
    #[serde(with = "alloy_serde::quantity")]
    pub min_gas_price: u128,
    pub mining: config::Mining,
    pub network_id: u64,
}

impl<HardforkT> TryFrom<Config> for edr_provider::ProviderConfig<HardforkT>
where
    HardforkT: for<'s> TryFrom<&'s str, Error = ()> + Into<l1::SpecId>,
{
    type Error = napi::Error;

    fn try_from(value: Config) -> Result<Self, Self::Error> {
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
                            let hardfork = HardforkT::try_from(&hardfork).map_err(|()| {
                                napi::Error::new(
                                    napi::Status::InvalidArg,
                                    format!("Unknown hardfork: {hardfork}"),
                                )
                            })?;

                            Ok((condition, hardfork))
                        },
                    )
                    .collect::<napi::Result<_>>()?;

                Ok((chain_id, Activations::new(activations)))
            })
            .collect::<napi::Result<_>>()?;

        let hardfork = HardforkT::try_from(&value.hardfork).map_err(|()| {
            napi::Error::new(
                napi::Status::InvalidArg,
                format!("Unknown hardfork: {}", value.hardfork),
            )
        })?;

        Ok(Self {
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
            genesis_state: value.genesis_state,
            hardfork,
            initial_base_fee_per_gas: value.initial_base_fee_per_gas,
            initial_blob_gas: value.initial_blob_gas,
            initial_date: value.initial_date,
            initial_parent_beacon_block_root: value.initial_parent_beacon_block_root,
            min_gas_price: value.min_gas_price,
            mining: value.mining,
            network_id: value.network_id,
        })
    }
}
