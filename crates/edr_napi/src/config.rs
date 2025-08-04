use core::fmt::{Debug, Display};
use std::{
    num::NonZeroU64,
    path::PathBuf,
    time::{Duration, SystemTime},
};

use edr_coverage::reporter::SyncOnCollectedCoverageCallback;
use edr_eth::{
    signature::{secret_key_from_str, SecretKey},
    Bytes, HashMap, HashSet,
};
use napi::{
    bindgen_prelude::{BigInt, Promise, Reference, Uint8Array},
    threadsafe_function::{
        ErrorStrategy, ThreadSafeCallContext, ThreadsafeFunction, ThreadsafeFunctionCallMode,
    },
    tokio::runtime,
    Either, JsFunction, JsString, JsStringUtf8,
};
use napi_derive::napi;

use crate::{account::AccountOverride, block::BlobGas, cast::TryCast, precompile::Precompile};

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
pub struct CodeCoverageConfig {
    /// The callback to be called when coverage has been collected.
    ///
    /// The callback receives an array of unique coverage hit markers (i.e. no
    /// repetition) per transaction.
    ///
    /// Exceptions thrown in the callback will be propagated to the original
    /// caller.
    #[napi(ts_type = "(coverageHits: Uint8Array[]) => Promise<void>")]
    pub on_collected_coverage_callback: JsFunction,
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
    pub interval: Option<Either<BigInt, IntervalRange>>,
    pub mem_pool: MemPoolConfig,
}

/// Configuration for runtime observability.
#[napi(object)]
pub struct ObservabilityConfig {
    /// If present, configures runtime observability to collect code coverage.
    pub code_coverage: Option<CodeCoverageConfig>,
}

/// Configuration for a provider
#[napi(object)]
pub struct ProviderConfig {
    /// Whether to allow blocks with the same timestamp
    pub allow_blocks_with_same_timestamp: bool,
    /// Whether to allow unlimited contract size
    pub allow_unlimited_contract_size: bool,
    /// Whether to return an `Err` when `eth_call` fails
    pub bail_on_call_failure: bool,
    /// Whether to return an `Err` when a `eth_sendTransaction` fails
    pub bail_on_transaction_failure: bool,
    /// The gas limit of each block
    pub block_gas_limit: BigInt,
    /// The chain ID of the blockchain
    pub chain_id: BigInt,
    /// The address of the coinbase
    pub coinbase: Uint8Array,
    /// The configuration for forking a blockchain. If not provided, a local
    /// blockchain will be created
    pub fork: Option<ForkConfig>,
    /// The genesis state of the blockchain
    pub genesis_state: Vec<AccountOverride>,
    /// The hardfork of the blockchain
    pub hardfork: String,
    /// The initial base fee per gas of the blockchain. Required for EIP-1559
    /// transactions and later
    pub initial_base_fee_per_gas: Option<BigInt>,
    /// The initial blob gas of the blockchain. Required for EIP-4844
    pub initial_blob_gas: Option<BlobGas>,
    /// The initial date of the blockchain, in seconds since the Unix epoch
    pub initial_date: Option<BigInt>,
    /// The initial parent beacon block root of the blockchain. Required for
    /// EIP-4788
    pub initial_parent_beacon_block_root: Option<Uint8Array>,
    /// The minimum gas price of the next block.
    pub min_gas_price: BigInt,
    /// The configuration for the miner
    pub mining: MiningConfig,
    /// The network ID of the blockchain
    pub network_id: BigInt,
    /// The configuration for the provider's observability
    pub observability: ObservabilityConfig,
    // Using JsString here as it doesn't have `Debug`, `Display` and `Serialize` implementation
    // which prevents accidentally leaking the secret keys to error messages and logs.
    /// Secret keys of owned accounts
    pub owned_accounts: Vec<JsString>,
    /// Overrides for precompiles
    pub precompile_overrides: Vec<Reference<Precompile>>,
}

impl TryFrom<ForkConfig> for edr_provider::ForkConfig<String> {
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
                                            }) => edr_evm::hardfork::ForkCondition::Block(
                                                block_number.try_cast()?,
                                            ),
                                            Either::B(HardforkActivationByTimestamp {
                                                timestamp,
                                            }) => edr_evm::hardfork::ForkCondition::Timestamp(
                                                timestamp.try_cast()?,
                                            ),
                                        };

                                        Ok(edr_evm::hardfork::Activation {
                                            condition,
                                            hardfork,
                                        })
                                    },
                                )
                                .collect::<napi::Result<Vec<_>>>()
                                .map(edr_evm::hardfork::Activations::new)
                                    })
                                    .transpose()?;

                            let chain_config = edr_evm::hardfork::ChainOverride {
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

impl From<MemPoolConfig> for edr_provider::MemPoolConfig {
    fn from(value: MemPoolConfig) -> Self {
        Self {
            order: value.order.into(),
        }
    }
}

impl From<MineOrdering> for edr_evm::MineOrdering {
    fn from(value: MineOrdering) -> Self {
        match value {
            MineOrdering::Fifo => Self::Fifo,
            MineOrdering::Priority => Self::Priority,
        }
    }
}

impl TryFrom<MiningConfig> for edr_provider::MiningConfig {
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
                                "Interval must be greater than 0",
                            )
                        })?;

                        edr_provider::IntervalConfig::Fixed(interval)
                    }
                    Either::B(IntervalRange { min, max }) => edr_provider::IntervalConfig::Range {
                        min: min.try_cast()?,
                        max: max.try_cast()?,
                    },
                };

                napi::Result::Ok(interval)
            })
            .transpose()?;

        Ok(Self {
            auto_mine: value.auto_mine,
            interval,
            mem_pool,
        })
    }
}

impl ObservabilityConfig {
    /// Resolves the instance, converting it to a
    /// [`edr_provider::observability::Config`].
    pub fn resolve(
        self,
        env: &napi::Env,
        runtime: runtime::Handle,
    ) -> napi::Result<edr_provider::observability::Config> {
        let on_collected_coverage_fn = self
            .code_coverage
            .map(
                |code_coverage| -> napi::Result<Box<dyn SyncOnCollectedCoverageCallback>> {
                    let mut on_collected_coverage_callback: ThreadsafeFunction<
                        _,
                        ErrorStrategy::Fatal,
                    > = code_coverage
                        .on_collected_coverage_callback
                        .create_threadsafe_function(
                            0,
                            |ctx: ThreadSafeCallContext<HashSet<Bytes>>| {
                                let hits = ctx
                                    .env
                                    .create_array_with_length(ctx.value.len())
                                    .and_then(|mut hits| {
                                        for (idx, hit) in ctx.value.into_iter().enumerate() {
                                            ctx.env
                                                .create_buffer_with_data(hit.to_vec())
                                                .and_then(|hit| {
                                                    let idx = u32::try_from(idx).unwrap_or_else(|_| panic!("Number of hits should not exceed '{}'",
                                                        u32::MAX));

                                                    hits.set_element(idx, hit.into_raw())
                                                })?;
                                        }
                                        Ok(hits)
                                    })?;

                                Ok(vec![hits])
                            },
                        )?;

                    // Maintain a weak reference to the function to avoid blocking the event loop
                    // from exiting.
                    on_collected_coverage_callback.unref(env)?;

                    let on_collected_coverage_fn: Box<dyn SyncOnCollectedCoverageCallback> =
                        Box::new(move |hits| {
                            let runtime = runtime.clone();

                            let (sender, receiver) = std::sync::mpsc::channel();

                            let status = on_collected_coverage_callback
                                .call_with_return_value(hits, ThreadsafeFunctionCallMode::Blocking, move |result: Promise<()>| {
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
                                });

                            assert_eq!(status, napi::Status::Ok);

                            let () = receiver.recv().expect("Receive can only fail if the channel is closed")?;

                            Ok(())
                        });

                    Ok(on_collected_coverage_fn)
                },
            )
            .transpose()?;

        Ok(edr_provider::observability::Config {
            on_collected_coverage_fn,
            ..edr_provider::observability::Config::default()
        })
    }
}

impl ProviderConfig {
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
                use edr_eth::signature::DangerousSecretKeyStr;

                static_assertions::assert_not_impl_all!(JsString: Debug, Display, serde::Serialize);
                static_assertions::assert_not_impl_all!(JsStringUtf8: Debug, Display, serde::Serialize);
                // `SecretKey` has `Debug` implementation, but it's opaque (only shows the
                // type name)
                static_assertions::assert_not_impl_any!(SecretKey: Display, serde::Serialize);

                let secret_key = secret_key.into_utf8()?;
                // This is the only place in production code where it's allowed to use
                // `DangerousSecretKeyStr`.
                #[allow(deprecated)]
                let secret_key_str = DangerousSecretKeyStr(secret_key.as_str()?);
                let secret_key: SecretKey = secret_key_from_str(secret_key_str)
                    .map_err(|error| napi::Error::new(napi::Status::InvalidArg, error))?;

                Ok(secret_key)
            })
            .collect::<napi::Result<Vec<_>>>()?;

        let block_gas_limit =
            NonZeroU64::new(self.block_gas_limit.try_cast()?).ok_or_else(|| {
                napi::Error::new(
                    napi::Status::GenericFailure,
                    "Block gas limit must be greater than 0",
                )
            })?;

        let genesis_state = self
            .genesis_state
            .into_iter()
            .map(TryInto::try_into)
            .collect::<napi::Result<HashMap<edr_eth::Address, edr_provider::AccountOverride>>>()?;

        let precompile_overrides = self
            .precompile_overrides
            .into_iter()
            .map(|precompile| precompile.to_tuple())
            .collect();

        Ok(edr_napi_core::provider::Config {
            allow_blocks_with_same_timestamp: self.allow_blocks_with_same_timestamp,
            allow_unlimited_contract_size: self.allow_unlimited_contract_size,
            bail_on_call_failure: self.bail_on_call_failure,
            bail_on_transaction_failure: self.bail_on_transaction_failure,
            block_gas_limit,
            chain_id: self.chain_id.try_cast()?,
            coinbase: self.coinbase.try_cast()?,
            fork: self.fork.map(TryInto::try_into).transpose()?,
            genesis_state,
            hardfork: self.hardfork,
            initial_base_fee_per_gas: self
                .initial_base_fee_per_gas
                .map(TryCast::try_cast)
                .transpose()?,
            initial_blob_gas: self.initial_blob_gas.map(TryInto::try_into).transpose()?,
            initial_date: self
                .initial_date
                .map(|date| {
                    let elapsed_since_epoch = Duration::from_secs(date.try_cast()?);
                    napi::Result::Ok(SystemTime::UNIX_EPOCH + elapsed_since_epoch)
                })
                .transpose()?,
            initial_parent_beacon_block_root: self
                .initial_parent_beacon_block_root
                .map(TryCast::try_cast)
                .transpose()?,
            mining: self.mining.try_into()?,
            min_gas_price: self.min_gas_price.try_cast()?,
            network_id: self.network_id.try_cast()?,
            observability: self.observability.resolve(env, runtime)?,
            owned_accounts,
            precompile_overrides,
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
