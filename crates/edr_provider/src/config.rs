use std::{num::NonZeroU64, path::PathBuf, time::SystemTime};

use edr_eth::{Address, B256, Bytecode, ChainId, HashMap, U256, block::BlobGas};
use edr_evm::{MineOrdering, hardfork::ChainConfig, precompile::PrecompileFn, state::EvmStorage};
use rand::Rng;
use serde::{Deserialize, Serialize};

use crate::{observability, requests::IntervalConfig as IntervalConfigRequest};

/// Configuration of an account and its storage.
///
/// Similar to `edr_eth::Account` but without the `status` field.
#[derive(Clone, Debug, Default, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AccountOverride {
    /// If present, the overwriting balance.
    pub balance: Option<U256>,
    /// If present, the overwriting nonce.
    pub nonce: Option<u64>,
    /// If present, the overwriting code.
    pub code: Option<Bytecode>,
    /// If present, the overwriting storage
    pub storage: Option<EvmStorage>,
}

/// Configuration for forking a blockchain
#[derive(Clone, Debug, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Fork {
    pub block_number: Option<u64>,
    pub cache_dir: PathBuf,
    pub http_headers: Option<std::collections::HashMap<String, String>>,
    pub url: String,
}

/// Configuration for interval mining.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all_fields = "camelCase")]
pub enum Interval {
    Fixed(NonZeroU64),
    Range { min: u64, max: u64 },
}

impl Interval {
    /// Generates a (random) interval based on the configuration.
    pub fn generate_interval(&self) -> u64 {
        match self {
            Interval::Fixed(interval) => interval.get(),
            Interval::Range { min, max } => rand::thread_rng().gen_range(*min..=*max),
        }
    }
}

/// An error that occurs when trying to convert [`IntervalConfigRequest`] to an
/// `Option<IntervalConfig>`.
#[derive(Debug, thiserror::Error)]
pub enum IntervalConfigConversionError {
    /// The minimum value in the range is greater than the maximum value.
    #[error("Minimum value in range is greater than maximum value")]
    MinGreaterThanMax,
}

impl TryInto<Option<Interval>> for IntervalConfigRequest {
    type Error = IntervalConfigConversionError;

    fn try_into(self) -> Result<Option<Interval>, Self::Error> {
        match self {
            Self::FixedOrDisabled(0) => Ok(None),
            Self::FixedOrDisabled(value) => {
                // Zero implies disabled
                Ok(NonZeroU64::new(value).map(Interval::Fixed))
            }
            Self::Range([min, max]) => {
                if max >= min {
                    Ok(Some(Interval::Range { min, max }))
                } else {
                    Err(IntervalConfigConversionError::MinGreaterThanMax)
                }
            }
        }
    }
}

/// Configuration for the provider's mempool.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MemPool {
    pub order: MineOrdering,
}

/// Configuration for the provider's miner.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Mining {
    pub auto_mine: bool,
    pub interval: Option<Interval>,
    pub mem_pool: MemPool,
}

/// Configuration for the provider
#[derive(Clone, Debug)]
pub struct Provider<HardforkT> {
    pub allow_blocks_with_same_timestamp: bool,
    pub allow_unlimited_contract_size: bool,
    /// Whether to return an `Err` when `eth_call` fails
    pub bail_on_call_failure: bool,
    /// Whether to return an `Err` when a `eth_sendTransaction` fails
    pub bail_on_transaction_failure: bool,
    pub block_gas_limit: NonZeroU64,
    pub chain_id: ChainId,
    pub chains: HashMap<ChainId, ChainConfig<HardforkT>>,
    pub coinbase: Address,
    pub fork: Option<Fork>,
    pub genesis_state: HashMap<Address, AccountOverride>,
    pub hardfork: HardforkT,
    pub initial_base_fee_per_gas: Option<u128>,
    pub initial_blob_gas: Option<BlobGas>,
    pub initial_date: Option<SystemTime>,
    pub initial_parent_beacon_block_root: Option<B256>,
    pub min_gas_price: u128,
    pub mining: Mining,
    pub network_id: u64,
    pub observability: observability::Config,
    pub owned_accounts: Vec<k256::SecretKey>,
    pub precompile_overrides: HashMap<Address, PrecompileFn>,
}

impl Default for MemPool {
    fn default() -> Self {
        Self {
            order: MineOrdering::Priority,
        }
    }
}

impl Default for Mining {
    fn default() -> Self {
        Self {
            auto_mine: true,
            interval: None,
            mem_pool: MemPool::default(),
        }
    }
}
