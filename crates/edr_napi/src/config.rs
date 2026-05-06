use core::fmt::{Debug, Display};
use std::{
    num::NonZeroU64,
    path::PathBuf,
    time::{Duration, SystemTime},
};

use edr_coverage::reporter::SyncOnCollectedCoverageCallback;
use edr_eip1559::{BaseFeeActivation, ConstantBaseFeeParams};
use edr_gas_report::SyncOnCollectedGasReportCallback;
use edr_napi_core::provider::ConfigOption;
use edr_primitives::{Bytes, HashMap, HashSet};
use edr_signer::{secret_key_from_str, SecretKey};
use napi::{
    bindgen_prelude::{BigInt, FromNapiValue, Function, Promise, Reference, Uint8Array},
    threadsafe_function::{ThreadsafeCallContext, ThreadsafeFunctionCallMode},
    tokio::runtime,
    Either, Env,
};
use napi_derive::napi;

use crate::{
    account::AccountOverride, block::BlobGas, cast::TryCast, gas_report::GasReport,
    logger::LoggerConfig, precompile::Precompile, solidity_tests::config::IncludeTraces,
    subscription::SubscriptionConfig,
};

/// Configuration for EIP-1559 parameters
#[napi(object)]
pub struct BaseFeeParamActivation {
    pub activation: Either<BaseFeeActivationByBlockNumber, BaseFeeActivationByHardfork>,
    pub max_change_denominator: BigInt,
    pub elasticity_multiplier: BigInt,
}

#[napi(object)]
pub struct BaseFeeActivationByBlockNumber {
    /// The block number at which the `base_fee_params` is activated
    pub block_number: BigInt,
}
#[napi(object)]
pub struct BaseFeeActivationByHardfork {
    /// The hardfork at which the `base_fee_params` is activated
    pub hardfork: String,
}

impl TryFrom<BaseFeeParamActivation> for (BaseFeeActivation<String>, ConstantBaseFeeParams) {
    type Error = napi::Error;

    fn try_from(value: BaseFeeParamActivation) -> Result<Self, Self::Error> {
        let base_fee_params = ConstantBaseFeeParams {
            max_change_denominator: value.max_change_denominator.try_cast()?,
            elasticity_multiplier: value.elasticity_multiplier.try_cast()?,
        };

        match value.activation {
            Either::A(BaseFeeActivationByBlockNumber { block_number }) => {
                let activation_block_number: u64 = block_number.try_cast()?;
                Ok((
                    BaseFeeActivation::BlockNumber(activation_block_number),
                    base_fee_params,
                ))
            }
            Either::B(BaseFeeActivationByHardfork { hardfork }) => {
                Ok((BaseFeeActivation::Hardfork(hardfork), base_fee_params))
            }
        }
    }
}

/// Specification of a chain with possible overrides.
#[napi(object)]
pub struct ChainOverride {
    /// The chain ID
    pub chain_id: BigInt,
    /// The chain's name
    pub name: String,
    /// If present, overrides for the chain's supported hardforks
    pub hardfork_activation_overrides: Option<Vec<HardforkActivation>>,
}

/// Configuration for a code coverage reporter.
#[napi(object)]
pub struct CodeCoverageConfig<'env> {
    /// The callback to be called when coverage has been collected.
    ///
    /// The callback receives an array of unique coverage hit markers (i.e. no
    /// repetition) per transaction.
    ///
    /// Exceptions thrown in the callback will be propagated to the original
    /// caller.
    #[napi(ts_type = "(coverageHits: Uint8Array[]) => Promise<void>")]
    pub on_collected_coverage_callback: Function<'env, Vec<Uint8Array>, Promise<()>>,
}

#[napi(object)]
pub struct GasReportConfig<'env> {
    /// Gas reports are collected after a block is mined or `eth_call` is
    /// executed.
    ///
    /// Exceptions thrown in the callback will be propagated to the original
    /// caller.
    #[napi(ts_type = "(gasReport: GasReport) => Promise<void>")]
    pub on_collected_gas_report_callback: Function<'env, GasReport, Promise<()>>,
}

/// Configuration for forking a blockchain
#[napi(object)]
pub struct ForkConfig {
    /// The block number to fork from. If not provided, the latest safe block is
    /// used.
    pub block_number: Option<BigInt>,
    /// The directory to cache remote JSON-RPC responses
    pub cache_dir: Option<String>,
    /// Overrides for the configuration of chains.
    pub chain_overrides: Option<Vec<ChainOverride>>,
    /// The HTTP headers to use when making requests to the JSON-RPC endpoint
    pub http_headers: Option<Vec<HttpHeader>>,
    /// The URL of the JSON-RPC endpoint to fork from
    pub url: String,
}

#[napi(object)]
pub struct HttpHeader {
    pub name: String,
    pub value: String,
}

/// Configuration for a hardfork activation
#[napi(object)]
pub struct HardforkActivation {
    /// The condition for the hardfork activation
    pub condition: Either<HardforkActivationByBlockNumber, HardforkActivationByTimestamp>,
    /// The activated hardfork
    pub hardfork: String,
}

#[napi(object)]
pub struct HardforkActivationByBlockNumber {
    /// The block number at which the hardfork is activated
    pub block_number: BigInt,
}

#[napi(object)]
pub struct HardforkActivationByTimestamp {
    /// The timestamp at which the hardfork is activated
    pub timestamp: BigInt,
}

/// Controls the gas estimation strategy used by `eth_estimateGas`.
#[napi]
pub enum GasEstimationMode {
    /// Estimates the minimum gas required for the top-level call to succeed.
    TopLevelSuccess,
    /// Estimates the minimum gas required for the top-level call to succeed
    /// without any internal sub-call running out of gas.
    NoInternalOutOfGas,
}

impl From<GasEstimationMode> for edr_provider::config::GasEstimationMode {
    fn from(value: GasEstimationMode) -> Self {
        match value {
            GasEstimationMode::TopLevelSuccess => Self::TopLevelSuccess,
            GasEstimationMode::NoInternalOutOfGas => Self::NoInternalOutOfGas,
        }
    }
}

#[napi(string_enum)]
#[doc = "The type of ordering to use when selecting blocks to mine."]
pub enum MineOrdering {
    #[doc = "Insertion order"]
    Fifo,
    #[doc = "Effective miner fee"]
    Priority,
}

/// Configuration for the provider's mempool.
#[napi(object)]
pub struct MemPoolConfig {
    pub order: MineOrdering,
}

#[napi(object)]
pub struct IntervalRange {
    pub min: BigInt,
    pub max: BigInt,
}

/// Configuration for the provider's miner.
#[napi(object)]
pub struct MiningConfig {
    pub auto_mine: bool,
    /// The block gas limit to use for mining a block.
    ///
    /// When not set, enforcement of the block gas limit is disabled in the mem
    /// pool, miner, and REVM.
    pub block_gas_limit: Option<BigInt>,
    pub interval: Option<Either<BigInt, IntervalRange>>,
    pub mem_pool: MemPoolConfig,
}

/// Configuration for a locally mined blockchain.
#[napi(object)]
pub struct LocalConfig {
    /// The blob gas used for the genesis block, introduced in [EIP-4844].
    ///
    /// [EIP-4844]: https://eips.ethereum.org/EIPS/eip-4844
    pub genesis_blob_gas: Option<BlobGas>,
    /// The block gas limit of the genesis block.
    pub genesis_block_gas_limit: BigInt,
    /// The date, in seconds since the Unix epoch, of the genesis block.
    pub genesis_block_time: Option<BigInt>,
}

impl TryFrom<LocalConfig> for edr_provider::config::Local {
    type Error = napi::Error;

    fn try_from(value: LocalConfig) -> Result<Self, Self::Error> {
        let genesis_blob_gas = value.genesis_blob_gas.map(TryInto::try_into).transpose()?;

        let genesis_block_gas_limit = value.genesis_block_gas_limit.try_cast()?;
        let genesis_block_gas_limit =
            NonZeroU64::new(genesis_block_gas_limit).ok_or_else(|| {
                napi::Error::new(
                    napi::Status::GenericFailure,
                    "Genesis block gas limit must not be zero",
                )
            })?;

        let genesis_block_time = value
            .genesis_block_time
            .map(|date| {
                let elapsed_since_epoch = Duration::from_secs(date.try_cast()?);
                napi::Result::Ok(SystemTime::UNIX_EPOCH + elapsed_since_epoch)
            })
            .transpose()?;

        Ok(Self {
            genesis_blob_gas,
            genesis_block_gas_limit,
            genesis_block_time,
        })
    }
}

/// Configuration for runtime observability.
#[napi(object)]
pub struct ObservabilityConfig<'env> {
    /// If present, configures runtime observability to collect code coverage.
    pub code_coverage: Option<CodeCoverageConfig<'env>>,
    /// If present, configures runtime observability to collect gas reports.
    pub gas_report: Option<GasReportConfig<'env>>,
    /// Controls when to include call traces in the results of transaction
    /// execution.
    ///
    /// Defaults to `IncludeTraces.None`.
    pub include_call_traces: Option<IncludeTraces>,
}

/// Configuration for a provider
#[napi(object)]
pub struct ProviderConfig<'env> {
    /// Whether to allow blocks with the same timestamp
    pub allow_blocks_with_same_timestamp: bool,
    /// Whether to allow unlimited contract size
    pub allow_unlimited_contract_size: bool,
    /// Whether to return an `Err` when `eth_call` fails
    pub bail_on_call_failure: bool,
    /// Whether to return an `Err` when a `eth_sendTransaction` fails
    pub bail_on_transaction_failure: bool,
    /// EIP-1559 base fee parameters activations to be used to calculate the
    /// block base fee.
    ///
    /// Provide an ordered list of `base_fee_params` to be
    /// used starting from the specified activation point (hardfork or block
    /// number).
    /// If not provided, the default values from the chain spec
    /// will be used.
    pub base_fee_config: Option<Vec<BaseFeeParamActivation>>,
    /// The chain ID of the blockchain
    pub chain_id: BigInt,
    /// The address of the coinbase
    pub coinbase: Uint8Array,
    /// The default transaction gas limit to use for RPC call and transaction
    /// requests that do not specify a `gas` value.
    pub default_transaction_gas_limit: BigInt,
    /// The gas estimation mode to use for `eth_estimateGas`. Defaults to
    /// `GasEstimationMode::TopLevelSuccess` if not set.
    pub gas_estimation_mode: Option<GasEstimationMode>,
    /// The genesis state of the blockchain
    pub genesis_state: Vec<AccountOverride>,
    /// The hardfork of the blockchain
    pub hardfork: String,
    /// The initial base fee per gas of the blockchain. Required for EIP-1559
    /// transactions and later
    pub initial_base_fee_per_gas: Option<BigInt>,
    /// The initial parent beacon block root of the blockchain. Required for
    /// EIP-4788
    pub initial_parent_beacon_block_root: Option<Uint8Array>,
    /// The minimum gas price of the next block.
    pub min_gas_price: BigInt,
    /// The configuration for the miner
    pub mining: MiningConfig,
    /// The network configuration for the provider.
    pub network: Either<ForkConfig, LocalConfig>,
    /// The network ID of the blockchain
    pub network_id: BigInt,
    /// The configuration for the provider's observability
    pub observability: ObservabilityConfig<'env>,
    // Using `OpaqueString` here as it doesn't implement `Debug`, `Display` or
    // `Serialize`, which prevents accidentally leaking the secret keys to error
    // messages and logs.
    /// Secret keys of owned accounts
    pub owned_accounts: Vec<OpaqueString>,
    /// Overrides for precompiles
    pub precompile_overrides: Vec<Reference<Precompile>>,
    /// Transaction gas cap, introduced in [EIP-7825].
    ///
    /// Integer values should be larger than zero.
    ///
    /// When `false`, enforcement of the transaction gas cap is disabled and
    /// transactions with any `gas` value are accepted by the mempool and
    /// executed without REVM's transaction gas cap check.
    ///
    /// When not set, a hardfork-specific default value will be used.
    ///
    /// [EIP-7825]: https://eips.ethereum.org/EIPS/eip-7825
    #[napi(ts_type = "bigint | false")]
    pub transaction_gas_cap: Option<Either<BigInt, bool>>,
}

impl TryFrom<ForkConfig> for edr_provider::config::Fork<String> {
    type Error = napi::Error;

    fn try_from(value: ForkConfig) -> Result<Self, Self::Error> {
        let block_number: Option<u64> = value.block_number.map(TryCast::try_cast).transpose()?;

        let cache_dir = PathBuf::from(
            value
                .cache_dir
                .unwrap_or(edr_defaults::CACHE_DIR.to_owned()),
        );

        let chain_overrides = value
            .chain_overrides
            .map(|chain_overrides| {
                chain_overrides
                    .into_iter()
                    .map(
                        |ChainOverride {
                             chain_id,
                             name,
                             hardfork_activation_overrides,
                         }| {
                            let hardfork_activation_overrides =
                                hardfork_activation_overrides
                                    .map(|hardfork_activations| {
                                        hardfork_activations
                                .into_iter()
                                .map(
                                    |HardforkActivation {
                                         condition,
                                         hardfork,
                                     }| {
                                        let condition = match condition {
                                            Either::A(HardforkActivationByBlockNumber {
                                                block_number,
                                            }) => edr_chain_config::ForkCondition::Block(
                                                block_number.try_cast()?,
                                            ),
                                            Either::B(HardforkActivationByTimestamp {
                                                timestamp,
                                            }) => edr_chain_config::ForkCondition::Timestamp(
                                                timestamp.try_cast()?,
                                            ),
                                        };

                                        Ok(edr_chain_config::HardforkActivation {
                                            condition,
                                            hardfork,
                                        })
                                    },
                                )
                                .collect::<napi::Result<Vec<_>>>()
                                .map(edr_chain_config::HardforkActivations::new)
                                    })
                                    .transpose()?;

                            let chain_config = edr_chain_config::ChainOverride {
                                name,
                                hardfork_activation_overrides,
                            };

                            let chain_id = chain_id.try_cast()?;
                            Ok((chain_id, chain_config))
                        },
                    )
                    .collect::<napi::Result<_>>()
            })
            .transpose()?;

        let http_headers = value.http_headers.map(|http_headers| {
            http_headers
                .into_iter()
                .map(|HttpHeader { name, value }| (name, value))
                .collect()
        });

        Ok(Self {
            block_number,
            cache_dir,
            chain_overrides: chain_overrides.unwrap_or_default(),
            http_headers,
            url: value.url,
        })
    }
}

impl From<MemPoolConfig> for edr_provider::config::MemPool {
    fn from(value: MemPoolConfig) -> Self {
        Self {
            order: value.order.into(),
        }
    }
}

impl From<MineOrdering> for edr_block_miner::MineOrdering {
    fn from(value: MineOrdering) -> Self {
        match value {
            MineOrdering::Fifo => Self::Fifo,
            MineOrdering::Priority => Self::Priority,
        }
    }
}

impl TryFrom<MiningConfig> for edr_provider::config::Mining {
    type Error = napi::Error;

    fn try_from(value: MiningConfig) -> Result<Self, Self::Error> {
        let mem_pool = value.mem_pool.into();

        let interval = value
            .interval
            .map(|interval| {
                let interval = match interval {
                    Either::A(interval) => {
                        let interval = interval.try_cast()?;
                        let interval = NonZeroU64::new(interval).ok_or_else(|| {
                            napi::Error::new(
                                napi::Status::GenericFailure,
                                "Interval must not be zero",
                            )
                        })?;

                        edr_provider::config::Interval::Fixed(interval)
                    }
                    Either::B(IntervalRange { min, max }) => {
                        edr_provider::config::Interval::Range {
                            min: min.try_cast()?,
                            max: max.try_cast()?,
                        }
                    }
                };

                napi::Result::Ok(interval)
            })
            .transpose()?;

        let block_gas_limit = value
            .block_gas_limit
            .map(|block_gas_limit| {
                block_gas_limit.try_cast().and_then(|block_gas_limit| {
                    NonZeroU64::new(block_gas_limit).ok_or_else(|| {
                        napi::Error::new(
                            napi::Status::GenericFailure,
                            "Block gas limit must not be zero",
                        )
                    })
                })
            })
            .transpose()?;

        Ok(Self {
            auto_mine: value.auto_mine,
            block_gas_limit,
            interval,
            mem_pool,
        })
    }
}

impl ObservabilityConfig<'_> {
    /// Resolves the instance, converting it to a
    /// [`edr_provider::observability::Config`].
    pub fn resolve(
        self,
        _env: &napi::Env,
        runtime: runtime::Handle,
    ) -> napi::Result<edr_provider::observability::Config> {
        let on_collected_coverage_fn = self
            .code_coverage
            .map(
                |code_coverage| -> napi::Result<Box<dyn SyncOnCollectedCoverageCallback>> {
                    let runtime = runtime.clone();

                    let on_collected_coverage_callback = std::sync::Arc::new(
                        code_coverage
                            .on_collected_coverage_callback
                            .build_threadsafe_function::<HashSet<Bytes>>()
                            .weak::<true>()
                            .build_callback(
                                |ctx: ThreadsafeCallContext<HashSet<Bytes>>| {
                                    let hits: Vec<Uint8Array> = ctx
                                        .value
                                        .into_iter()
                                        .map(|hit| Uint8Array::from(hit.to_vec()))
                                        .collect();

                                    Ok(hits)
                                },
                            )?,
                    );

                    let on_collected_coverage_fn: Box<dyn SyncOnCollectedCoverageCallback> =
                        Box::new(move |hits| {
                            let runtime = runtime.clone();

                            let (sender, receiver) = std::sync::mpsc::channel();

                            let status = on_collected_coverage_callback.call_with_return_value(
                                hits,
                                ThreadsafeFunctionCallMode::Blocking,
                                move |result: napi::Result<Promise<()>>, _env: Env| {
                                    let result = result?;
                                    // We spawn a background task to handle the async callback
                                    runtime.spawn(async move {
                                        let result = result.await;
                                        sender.send(result).map_err(|_error| {
                                            napi::Error::new(
                                                napi::Status::GenericFailure,
                                                "Failed to send result from on_collected_coverage_callback",
                                            )
                                        })
                                    });
                                    Ok(())
                                },
                            );

                            assert_eq!(status, napi::Status::Ok);

                            let () = receiver
                                .recv()
                                .expect("Receive can only fail if the channel is closed")?;

                            Ok(())
                        });

                    Ok(on_collected_coverage_fn)
                },
            )
            .transpose()?;
        let on_collected_gas_report_fn = self.gas_report.map(
            |gas_report| -> napi::Result<Box<dyn SyncOnCollectedGasReportCallback>> {
                let on_collected_gas_report_callback = std::sync::Arc::new(
                    gas_report
                        .on_collected_gas_report_callback
                        .build_threadsafe_function::<GasReport>()
                        .weak::<true>()
                        .build_callback(|ctx: ThreadsafeCallContext<GasReport>| Ok(ctx.value))?,
                );

                let on_collected_gas_report_fn: Box<dyn SyncOnCollectedGasReportCallback> =
                    Box::new(move |report| {
                        let runtime = runtime.clone();

                        let (sender, receiver) = std::sync::mpsc::channel();

                        // Convert the report to the N-API representation
                        let status = on_collected_gas_report_callback.call_with_return_value(
                            GasReport::from(report),
                            ThreadsafeFunctionCallMode::Blocking,
                            move |result: napi::Result<Promise<()>>, _env: Env| {
                                let result = result?;
                                // We spawn a background task to handle the async callback
                                runtime.spawn(async move {
                                    let result = result.await;
                                    sender.send(result).map_err(|_error| {
                                        napi::Error::new(
                                            napi::Status::GenericFailure,
                                            "Failed to send result from on_collected_gas_report_callback",
                                        )
                                    })
                                });
                                Ok(())
                            },
                        );

                        assert_eq!(status, napi::Status::Ok);

                        let () = receiver
                            .recv()
                            .expect("Receive can only fail if the channel is closed")?;

                        Ok(())
                    });

                Ok(on_collected_gas_report_fn)
            },
        ).transpose()?;

        let default_config = edr_provider::observability::Config::default();
        Ok(edr_provider::observability::Config {
            call_override: default_config.call_override,
            include_call_traces: self.include_call_traces.map_or(
                default_config.include_call_traces,
                edr_solidity::config::IncludeTraces::from,
            ),
            on_collected_coverage_fn,
            on_collected_gas_report_fn,
            verbose_raw_tracing: default_config.verbose_raw_tracing,
        })
    }
}

impl ProviderConfig<'_> {
    /// Resolves the instance to a [`edr_napi_core::provider::Config`].
    pub fn resolve(
        self,
        env: &napi::Env,
        runtime: runtime::Handle,
    ) -> napi::Result<edr_napi_core::provider::Config> {
        let owned_accounts = self
            .owned_accounts
            .into_iter()
            .map(|secret_key| {
                // This is the only place in production code where it's allowed to use
                // `DangerousSecretKeyStr`.
                #[allow(deprecated)]
                use edr_signer::DangerousSecretKeyStr;

                static_assertions::assert_not_impl_all!(OpaqueString: Debug, Display, serde::Serialize);
                // `SecretKey` has `Debug` implementation, but it's opaque (only shows the
                // type name)
                static_assertions::assert_not_impl_any!(SecretKey: Display, serde::Serialize);

                // This is the only place in production code where it's allowed to use
                // `DangerousSecretKeyStr`.
                #[allow(deprecated)]
                let secret_key_str = DangerousSecretKeyStr(secret_key.as_str());
                let secret_key: SecretKey = secret_key_from_str(secret_key_str)
                    .map_err(|error| napi::Error::new(napi::Status::InvalidArg, error))?;

                Ok(secret_key)
            })
            .collect::<napi::Result<Vec<_>>>()?;

        let base_fee_params: Option<Vec<(BaseFeeActivation<String>, ConstantBaseFeeParams)>> = self
            .base_fee_config
            .map(|vec| vec.into_iter().map(TryInto::try_into).collect())
            .transpose()?;

        let genesis_state = self
            .genesis_state
            .into_iter()
            .map(TryInto::try_into)
            .collect::<napi::Result<HashMap<edr_primitives::Address, edr_provider::AccountOverride>>>()?;

        let precompile_overrides = self
            .precompile_overrides
            .into_iter()
            .map(|precompile| precompile.to_tuple())
            .collect();

        let transaction_gas_cap = self
            .transaction_gas_cap.map_or(Ok(ConfigOption::Default), |transaction_gas_cap| match transaction_gas_cap {
                Either::A(gas_cap) => gas_cap.try_cast().map(ConfigOption::Custom),
                Either::B(disable) => if !disable {
                    Ok(ConfigOption::Disable)
                } else {
                    Err(napi::Error::new(napi::Status::InvalidArg, "Boolean value for `transactionGasCap` must be false to disable the transaction gas cap"))
                },
            })?;

        Ok(edr_napi_core::provider::Config {
            allow_blocks_with_same_timestamp: self.allow_blocks_with_same_timestamp,
            allow_unlimited_contract_size: self.allow_unlimited_contract_size,
            bail_on_call_failure: self.bail_on_call_failure,
            bail_on_transaction_failure: self.bail_on_transaction_failure,
            base_fee_params,
            chain_id: self.chain_id.try_cast()?,
            coinbase: self.coinbase.try_cast()?,
            default_transaction_gas_limit: self.default_transaction_gas_limit.try_cast().and_then(
                |default_transaction_gas_limit| {
                    NonZeroU64::new(default_transaction_gas_limit).ok_or_else(|| {
                        napi::Error::new(
                            napi::Status::GenericFailure,
                            "Default transaction gas limit must not be zero",
                        )
                    })
                },
            )?,
            gas_estimation_mode: self.gas_estimation_mode.map(Into::into),
            genesis_state,
            hardfork: self.hardfork,
            initial_base_fee_per_gas: self
                .initial_base_fee_per_gas
                .map(TryCast::try_cast)
                .transpose()?,
            initial_parent_beacon_block_root: self
                .initial_parent_beacon_block_root
                .map(TryCast::try_cast)
                .transpose()?,
            mining: self.mining.try_into()?,
            min_gas_price: self.min_gas_price.try_cast()?,
            network: match self.network {
                Either::A(fork_config) => {
                    let fork_config = fork_config.try_into()?;
                    edr_provider::config::Network::Fork(fork_config)
                }
                Either::B(local_config) => {
                    let local_config = local_config.try_into()?;
                    edr_provider::config::Network::Local(local_config)
                }
            },
            network_id: self.network_id.try_cast()?,
            observability: self.observability.resolve(env, runtime)?,
            owned_accounts,
            precompile_overrides,
            transaction_gas_cap,
        })
    }
}

/// Tracing config for Solidity stack trace generation.
#[napi(object)]
pub struct TracingConfigWithBuffers {
    /// Build information to use for decoding contracts. Either a Hardhat v2
    /// build info file that contains both input and output or a Hardhat v3
    /// build info file that doesn't contain output and a separate output file.
    pub build_infos: Option<Either<Vec<Uint8Array>, Vec<BuildInfoAndOutput>>>,
    /// Whether to ignore contracts whose name starts with "Ignored".
    pub ignore_contracts: Option<bool>,
}

impl From<TracingConfigWithBuffers> for edr_napi_core::solidity::config::TracingConfigWithBuffers {
    fn from(value: TracingConfigWithBuffers) -> Self {
        edr_napi_core::solidity::config::TracingConfigWithBuffers {
            build_infos: value.build_infos.map(|infos| match infos {
                Either::A(with_output) => Either::A(with_output),
                Either::B(separate_output) => Either::B(
                    separate_output
                        .into_iter()
                        .map(edr_napi_core::solidity::config::BuildInfoAndOutput::from)
                        .collect(),
                ),
            }),
            ignore_contracts: value.ignore_contracts,
        }
    }
}

/// Hardhat V3 build info where the compiler output is not part of the build
/// info file.
#[napi(object)]
pub struct BuildInfoAndOutput {
    /// The build info input file
    pub build_info: Uint8Array,
    /// The build info output file
    pub output: Uint8Array,
}

impl From<BuildInfoAndOutput> for edr_napi_core::solidity::config::BuildInfoAndOutput {
    fn from(value: BuildInfoAndOutput) -> Self {
        Self {
            build_info: value.build_info,
            output: value.output,
        }
    }
}

/// Result of [`resolve_configs`].
pub struct ConfigResolution {
    pub logger_config: edr_napi_core::logger::Config,
    pub provider_config: edr_napi_core::provider::Config,
    pub subscription_callback: edr_napi_core::subscription::Callback,
}

/// Helper function for resolving the provided N-API configs.
pub fn resolve_configs<'env>(
    env: &napi::Env,
    runtime: runtime::Handle,
    provider_config: ProviderConfig<'env>,
    logger_config: LoggerConfig<'env>,
    subscription_config: SubscriptionConfig<'env>,
) -> napi::Result<ConfigResolution> {
    let provider_config = provider_config.resolve(env, runtime)?;
    let logger_config = logger_config.resolve(env)?;

    let subscription_config = edr_napi_core::subscription::Config::from(subscription_config);
    let subscription_callback =
        edr_napi_core::subscription::Callback::new(env, subscription_config.subscription_callback)?;

    Ok(ConfigResolution {
        logger_config,
        provider_config,
        subscription_callback,
    })
}

/// Wrapper around a `String` that intentionally does NOT implement `Debug`,
/// `Display`, or `Serialize`, so that secret material such as private keys
/// cannot be accidentally written to logs, error messages, or telemetry.
///
/// In NAPI v2 the equivalent guarantee was provided by `napi::JsString`, which
/// is no longer the recommended public API in v3. Use this newtype anywhere
/// you would have used `Vec<JsString>` to carry sensitive strings.
pub struct OpaqueString(String);

impl OpaqueString {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl FromNapiValue for OpaqueString {
    unsafe fn from_napi_value(
        env: napi::sys::napi_env,
        napi_val: napi::sys::napi_value,
    ) -> napi::Result<Self> {
        let s = unsafe { String::from_napi_value(env, napi_val) }?;
        Ok(OpaqueString(s))
    }
}

impl napi::bindgen_prelude::ToNapiValue for OpaqueString {
    unsafe fn to_napi_value(
        env: napi::sys::napi_env,
        val: Self,
    ) -> napi::Result<napi::sys::napi_value> {
        unsafe { String::to_napi_value(env, val.0) }
    }
}

impl napi::bindgen_prelude::TypeName for OpaqueString {
    fn type_name() -> &'static str {
        "string"
    }

    fn value_type() -> napi::ValueType {
        napi::ValueType::String
    }
}

impl napi::bindgen_prelude::ValidateNapiValue for OpaqueString {}
