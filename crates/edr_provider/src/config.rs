use std::{num::NonZeroU64, path::PathBuf, time::SystemTime};

use edr_block_header::BlobGas;
use edr_block_miner::MineOrdering;
use edr_chain_config::ChainOverride;
use edr_eip1559::BaseFeeParams;
use edr_precompile::PrecompileFn;
use edr_primitives::{Address, Bytecode, ChainId, HashMap, B256, U256};
use edr_state_api::EvmStorage;
use rand::Rng;
use serde::{Deserialize, Serialize};

use crate::{
    observability::ObservabilityConfig, requests::IntervalConfig as IntervalConfigRequest,
};

/// Convenience type alias for [`ForkConfig`].
///
/// This allows usage like `edr_provider::config::Fork`.
pub type Fork<HardforkT> = ForkConfig<HardforkT>;

/// Convenience type alias for [`IntervalConfig`].
///
/// This allows usage like `edr_provider::config::Interval`.
pub type Interval = IntervalConfig;

/// Convenience type alias for [`LocalConfig`].
///
/// This allows usage like `edr_provider::config::Local`.
pub type Local = LocalConfig;

/// Convenience type alias for [`MemPoolConfig`].
///
/// This allows usage like `edr_provider::config::MemPool`.
pub type MemPool = MemPoolConfig;

/// Convenience type alias for [`MiningConfig`].
///
/// This allows usage like `edr_provider::config::Mining`.
pub type Mining = MiningConfig;

/// Convenience type alias for [`NetworkConfig`].
///
/// This allows usage like `edr_provider::config::Network`.
pub type Network<HardforkT> = NetworkConfig<HardforkT>;

/// Convenience type alias for [`ProviderConfig`].
///
/// This allows usage like `edr_provider::config::Provider`.
pub type Provider<HardforkT> = ProviderConfig<HardforkT>;

/// Specification of overrides for an account and its storage.
///
/// Similar to `edr_state_api::Account` but without the `status` field and
/// optional fields.
#[derive(Clone, Debug, Default, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AccountOverride {
    /// If present, the overwriting balance.
    pub balance: Option<U256>,
    /// If present, the overwriting nonce.
    pub nonce: Option<u64>,
    /// If present, the overwriting code.
    pub code: Option<Bytecode>,
    // TODO: Add support for this field
    // TODO: https://github.com/NomicFoundation/edr/issues/911
    /// If present, the overwriting storage
    pub storage: Option<EvmStorage>,
}

/// Configuration for the provider's network.
#[derive(Clone, Debug)]
pub enum NetworkConfig<HardforkT> {
    /// Forked blockchain.
    Fork(ForkConfig<HardforkT>),
    /// Locally mined blockchain.
    Local(LocalConfig),
}

impl<HardforkT> From<ForkConfig<HardforkT>> for NetworkConfig<HardforkT> {
    fn from(fork_config: ForkConfig<HardforkT>) -> Self {
        NetworkConfig::Fork(fork_config)
    }
}

impl<HardforkT> From<LocalConfig> for NetworkConfig<HardforkT> {
    fn from(local_config: LocalConfig) -> Self {
        NetworkConfig::Local(local_config)
    }
}

/// Configuration for a forked blockchain, which forks from an existing
/// blockchain at a specified block.
#[derive(Clone, Debug, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ForkConfig<HardforkT> {
    pub block_number: Option<u64>,
    pub cache_dir: PathBuf,
    pub chain_overrides: HashMap<ChainId, ChainOverride<HardforkT>>,
    pub http_headers: Option<std::collections::HashMap<String, String>>,
    pub url: String,
}

/// Configuration for a locally mined blockchain.
#[derive(Clone, Debug)]
pub struct LocalConfig {
    /// The blob gas used for the genesis block, introduced in [EIP-4844].
    ///
    /// [EIP-4844]: https://eips.ethereum.org/EIPS/eip-4844
    pub genesis_blob_gas: Option<BlobGas>,
    /// The block gas limit of the genesis block.
    pub genesis_block_gas_limit: NonZeroU64,
    /// The timestamp of the genesis block.
    pub genesis_block_time: Option<SystemTime>,
}

/// Configuration for interval mining.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all_fields = "camelCase")]
pub enum IntervalConfig {
    Fixed(NonZeroU64),
    Range { min: u64, max: u64 },
}

impl IntervalConfig {
    /// Generates a (random) interval based on the configuration.
    pub fn generate_interval(&self) -> u64 {
        match self {
            IntervalConfig::Fixed(interval) => interval.get(),
            IntervalConfig::Range { min, max } => rand::rng().random_range(*min..=*max),
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

impl TryInto<Option<IntervalConfig>> for IntervalConfigRequest {
    type Error = IntervalConfigConversionError;

    fn try_into(self) -> Result<Option<IntervalConfig>, Self::Error> {
        match self {
            Self::FixedOrDisabled(0) => Ok(None),
            Self::FixedOrDisabled(value) => {
                // Zero implies disabled
                Ok(NonZeroU64::new(value).map(IntervalConfig::Fixed))
            }
            Self::Range([min, max]) => {
                if max >= min {
                    Ok(Some(IntervalConfig::Range { min, max }))
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
pub struct MemPoolConfig {
    pub order: MineOrdering,
}

/// Configuration for the provider's miner.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MiningConfig {
    pub auto_mine: bool,
    /// The block gas limit to use for mining a block.
    ///
    /// When not set, enforcement of the block gas limit is disabled in the mem
    /// pool, miner, and REVM.
    pub block_gas_limit: Option<NonZeroU64>,
    pub interval: Option<IntervalConfig>,
    pub mem_pool: MemPoolConfig,
}

/// Configuration for the provider
#[derive(Clone, Debug)]
pub struct ProviderConfig<HardforkT> {
    pub allow_blocks_with_same_timestamp: bool,
    pub allow_unlimited_contract_size: bool,
    /// Whether to return an `Err` when `eth_call` fails
    pub bail_on_call_failure: bool,
    /// Whether to return an `Err` when a `eth_sendTransaction` fails
    pub bail_on_transaction_failure: bool,
    pub base_fee_params: Option<BaseFeeParams<HardforkT>>,
    pub chain_id: ChainId,
    pub coinbase: Address,
    /// The default transaction gas limit to use for RPC call and transaction
    /// requests that do not specify a `gas` value.
    pub default_transaction_gas_limit: NonZeroU64,
    pub genesis_state: HashMap<Address, AccountOverride>,
    pub hardfork: HardforkT,
    pub initial_base_fee_per_gas: Option<u128>,
    pub initial_parent_beacon_block_root: Option<B256>,
    pub min_gas_price: u128,
    pub mining: MiningConfig,
    pub network: NetworkConfig<HardforkT>,
    pub network_id: u64,
    pub observability: ObservabilityConfig,
    pub owned_accounts: Vec<k256::SecretKey>,
    pub precompile_overrides: HashMap<Address, PrecompileFn>,
    /// Transaction gas cap, introduced in [EIP-7825].
    ///
    /// When not set, enforcement of the transaction gas cap is disabled and
    /// transactions with any `gas` value are accepted by the mempool and
    /// executed without REVM's transaction gas cap check.
    ///
    /// [EIP-7825]: https://eips.ethereum.org/EIPS/eip-7825
    pub transaction_gas_cap: Option<u64>,
}

impl Default for MemPoolConfig {
    fn default() -> Self {
        Self {
            order: MineOrdering::Priority,
        }
    }
}

impl Default for MiningConfig {
    fn default() -> Self {
        Self {
            auto_mine: true,
            // SAFETY: literal is non-zero
            block_gas_limit: Some(unsafe { NonZeroU64::new_unchecked(60_000_000u64) }),
            interval: None,
            mem_pool: MemPoolConfig::default(),
        }
    }
}
