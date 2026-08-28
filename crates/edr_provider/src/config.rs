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
///
/// Every representable value yields a non-zero interval.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum IntervalConfig {
    /// Mine a block every `n` milliseconds.
    Fixed(NonZeroU64),
    /// Mine a block every `n` milliseconds, where `n` is drawn from the range
    /// anew before each block.
    Range(IntervalRangeConfig),
}

impl IntervalConfig {
    /// Generates a (random) interval in milliseconds, based on the
    /// configuration.
    pub fn generate_interval(&self) -> NonZeroU64 {
        match self {
            IntervalConfig::Fixed(interval) => *interval,
            IntervalConfig::Range(range) => range.generate_interval(),
        }
    }
}

impl From<NonZeroU64> for IntervalConfig {
    fn from(value: NonZeroU64) -> Self {
        Self::Fixed(value)
    }
}

impl From<IntervalRangeConfig> for IntervalConfig {
    fn from(value: IntervalRangeConfig) -> Self {
        Self::Range(value)
    }
}

/// An inclusive range of interval-mining intervals, in milliseconds.
///
/// Non-empty and free of zeroes by construction, so
/// [`IntervalRangeConfig::generate_interval`] can neither panic nor return
/// zero. Deserialization is validated through [`UncheckedIntervalRange`].
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(try_from = "UncheckedIntervalRange")]
pub struct IntervalRangeConfig {
    min: NonZeroU64,
    max: NonZeroU64,
}

impl IntervalRangeConfig {
    /// Constructs a new instance from inclusive bounds in milliseconds.
    ///
    /// Fails if `min` exceeds `max`.
    pub const fn new(
        min: NonZeroU64,
        max: NonZeroU64,
    ) -> Result<Self, IntervalConfigConversionError> {
        if min.get() > max.get() {
            Err(IntervalConfigConversionError::MinGreaterThanMax)
        } else {
            Ok(Self { min, max })
        }
    }

    /// Returns the inclusive lower bound, in milliseconds.
    pub const fn min(&self) -> NonZeroU64 {
        self.min
    }

    /// Returns the inclusive upper bound, in milliseconds.
    pub const fn max(&self) -> NonZeroU64 {
        self.max
    }

    /// Draws an interval uniformly from the range, in milliseconds. Both
    /// bounds are inclusive.
    pub fn generate_interval(&self) -> NonZeroU64 {
        // `min <= max` is an invariant of the type, so the subtraction cannot
        // underflow and `0..=span` is never empty.
        let span = self.max.get() - self.min.get();
        let offset = rand::rng().random_range(0..=span);

        self.min
            .checked_add(offset)
            .expect("`min + offset` is at most `max`")
    }
}

impl TryFrom<[u64; 2]> for IntervalRangeConfig {
    type Error = IntervalConfigConversionError;

    fn try_from([min, max]: [u64; 2]) -> Result<Self, Self::Error> {
        let min = NonZeroU64::new(min).ok_or(IntervalConfigConversionError::MinIsZero)?;

        // `min` is non-zero, so a zero `max` is smaller than `min`.
        let max = NonZeroU64::new(max).ok_or(IntervalConfigConversionError::MinGreaterThanMax)?;

        Self::new(min, max)
    }
}

/// The unvalidated wire representation of [`IntervalRangeConfig`].
///
/// Deserializing through this type prevents a scenario file from bypassing the
/// range's invariants.
#[derive(Deserialize)]
struct UncheckedIntervalRange {
    min: u64,
    max: u64,
}

impl TryFrom<UncheckedIntervalRange> for IntervalRangeConfig {
    type Error = IntervalConfigConversionError;

    fn try_from(value: UncheckedIntervalRange) -> Result<Self, Self::Error> {
        Self::try_from([value.min, value.max])
    }
}

/// An error that occurs when an interval-mining configuration is invalid.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum IntervalConfigConversionError {
    /// The minimum value in the range is zero.
    #[error("Minimum value in range must be greater than zero")]
    MinIsZero,
    /// The minimum value in the range is greater than the maximum value.
    #[error("Minimum value in range is greater than maximum value")]
    MinGreaterThanMax,
}

impl TryFrom<IntervalConfigRequest> for Option<IntervalConfig> {
    type Error = IntervalConfigConversionError;

    fn try_from(value: IntervalConfigRequest) -> Result<Self, Self::Error> {
        match value {
            IntervalConfigRequest::FixedOrDisabled(interval) => {
                // Zero implies disabled
                Ok(NonZeroU64::new(interval).map(IntervalConfig::Fixed))
            }
            IntervalConfigRequest::Range(bounds) => IntervalRangeConfig::try_from(bounds)
                .map(|range| Some(IntervalConfig::Range(range))),
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

/// Controls the gas estimation strategy used by `eth_estimateGas`.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum GasEstimationMode {
    /// Estimates the minimum gas required for the top-level call to succeed.
    #[default]
    TopLevelSuccess,
    /// Estimates the minimum gas required for the top-level call to succeed
    /// without any internal sub-call running out of gas.
    NoInternalOutOfGas,
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
    pub gas_estimation_mode: GasEstimationMode,
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

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn non_zero(value: u64) -> NonZeroU64 {
        NonZeroU64::new(value).expect("non-zero")
    }

    fn range(min: u64, max: u64) -> IntervalRangeConfig {
        IntervalRangeConfig::try_from([min, max]).expect("valid range")
    }

    #[test]
    fn interval_range_rejects_zero_minimum() {
        for bounds in [[0, 0], [0, 5]] {
            assert!(matches!(
                IntervalRangeConfig::try_from(bounds),
                Err(IntervalConfigConversionError::MinIsZero)
            ));
        }
    }

    #[test]
    fn interval_range_rejects_zero_maximum() {
        assert!(matches!(
            IntervalRangeConfig::try_from([1, 0]),
            Err(IntervalConfigConversionError::MinGreaterThanMax)
        ));
    }

    #[test]
    fn interval_range_rejects_min_greater_than_max() {
        for bounds in [[5, 1], [u64::MAX, 1]] {
            assert!(matches!(
                IntervalRangeConfig::try_from(bounds),
                Err(IntervalConfigConversionError::MinGreaterThanMax)
            ));
        }
    }

    #[test]
    fn interval_range_accepts_equal_bounds() {
        let range = range(5, 5);

        assert_eq!(range.min(), non_zero(5));
        assert_eq!(range.max(), non_zero(5));
    }

    #[test]
    fn interval_range_accepts_ascending_bounds() {
        let range = range(1, u64::MAX);

        assert_eq!(range.min(), non_zero(1));
        assert_eq!(range.max(), non_zero(u64::MAX));
    }

    /// A range whose bounds coincide is not collapsed into
    /// [`IntervalConfig::Fixed`], so that a configuration round-trips to the
    /// representation it was written as.
    #[test]
    fn interval_range_is_not_normalized_to_fixed() {
        assert_ne!(
            IntervalConfig::from(range(5, 5)),
            IntervalConfig::Fixed(non_zero(5))
        );
    }

    #[test]
    fn fixed_generates_the_configured_interval() {
        let config = IntervalConfig::Fixed(non_zero(7));

        assert_eq!(config.generate_interval(), non_zero(7));
    }

    #[test]
    fn interval_range_with_equal_bounds_is_deterministic() {
        let range = range(5, 5);

        for _ in 0..100 {
            assert_eq!(range.generate_interval(), non_zero(5));
        }
    }

    #[test]
    fn interval_range_generates_values_within_bounds() {
        let range = range(2, 4);

        for _ in 0..1_000 {
            let interval = range.generate_interval().get();
            assert!((2..=4).contains(&interval), "{interval} is out of bounds");
        }
    }

    /// Pins that both bounds are inclusive. Hardhat draws from a max-exclusive
    /// range; EDR does not.
    #[test]
    fn interval_range_includes_both_bounds() {
        let range = range(1, 2);

        let mut seen_min = false;
        let mut seen_max = false;
        for _ in 0..1_000 {
            match range.generate_interval().get() {
                1 => seen_min = true,
                2 => seen_max = true,
                other => panic!("{other} is out of bounds"),
            }
        }

        assert!(seen_min && seen_max);
    }

    /// Recorded scenario files carry a serialized [`MiningConfig`], so this
    /// representation cannot change without invalidating them.
    #[test]
    fn interval_config_serializes_to_externally_tagged_json() -> anyhow::Result<()> {
        let fixed = IntervalConfig::Fixed(non_zero(1000));
        assert_eq!(serde_json::to_value(&fixed)?, json!({ "Fixed": 1000 }));
        assert_eq!(
            serde_json::from_value::<IntervalConfig>(json!({ "Fixed": 1000 }))?,
            fixed
        );

        let ranged = IntervalConfig::from(range(1000, 5000));
        let ranged_json = json!({ "Range": { "min": 1000, "max": 5000 } });
        assert_eq!(serde_json::to_value(&ranged)?, ranged_json);
        assert_eq!(
            serde_json::from_value::<IntervalConfig>(ranged_json)?,
            ranged
        );

        Ok(())
    }

    #[test]
    fn mining_config_round_trips_an_interval_range() -> anyhow::Result<()> {
        let config = MiningConfig {
            auto_mine: true,
            block_gas_limit: None,
            interval: Some(IntervalConfig::from(range(1000, 5000))),
            mem_pool: MemPoolConfig {
                order: MineOrdering::Priority,
            },
        };

        assert_eq!(
            serde_json::to_value(&config)?,
            json!({
                "autoMine": true,
                "blockGasLimit": null,
                "interval": { "Range": { "min": 1000, "max": 5000 } },
                "memPool": { "order": "Priority" },
            })
        );

        let deserialized: MiningConfig = serde_json::from_value(serde_json::to_value(&config)?)?;
        assert_eq!(deserialized.interval, config.interval);

        Ok(())
    }

    #[test]
    fn deserializing_an_invalid_interval_range_fails() {
        for bounds in [json!({ "min": 0, "max": 5 }), json!({ "min": 5, "max": 1 })] {
            let error = serde_json::from_value::<IntervalConfig>(json!({ "Range": bounds }))
                .expect_err("the range is invalid");

            assert!(
                error.to_string().contains("Minimum value in range"),
                "unexpected error: {error}"
            );
        }
    }

    #[test]
    fn deserializing_a_zero_fixed_interval_fails() {
        assert!(serde_json::from_value::<IntervalConfig>(json!({ "Fixed": 0 })).is_err());
    }

    /// Scenario files are deserialized as a whole [`MiningConfig`], which is
    /// the path that must reject an invalid range.
    #[test]
    fn deserializing_a_mining_config_with_an_invalid_range_fails() {
        let json = json!({
            "autoMine": true,
            "blockGasLimit": null,
            "interval": { "Range": { "min": 0, "max": 0 } },
            "memPool": { "order": "Priority" },
        });

        assert!(serde_json::from_value::<MiningConfig>(json).is_err());
    }

    #[test]
    fn a_zero_request_interval_disables_interval_mining() -> anyhow::Result<()> {
        let config: Option<IntervalConfig> =
            IntervalConfigRequest::FixedOrDisabled(0).try_into()?;

        assert_eq!(config, None);

        Ok(())
    }

    #[test]
    fn a_request_interval_is_converted() -> anyhow::Result<()> {
        let fixed: Option<IntervalConfig> =
            IntervalConfigRequest::FixedOrDisabled(1000).try_into()?;
        assert_eq!(fixed, Some(IntervalConfig::Fixed(non_zero(1000))));

        let ranged: Option<IntervalConfig> =
            IntervalConfigRequest::Range([1000, 5000]).try_into()?;
        assert_eq!(ranged, Some(IntervalConfig::from(range(1000, 5000))));

        Ok(())
    }

    #[test]
    fn an_invalid_request_range_is_rejected() {
        for bounds in [[0, 0], [0, 5000], [5000, 1000]] {
            let result: Result<Option<IntervalConfig>, _> =
                IntervalConfigRequest::Range(bounds).try_into();

            assert!(result.is_err(), "{bounds:?} should be rejected");
        }
    }
}
