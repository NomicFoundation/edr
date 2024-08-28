use std::{collections::HashMap, fmt::Debug, path::PathBuf};

use alloy_primitives::{address, hex, Address, B256};
use edr_eth::U256;
use forge::{
    fork::CreateFork,
    inspectors::cheatcodes::CheatsConfigOptions,
    opts::{Env as EvmEnv, EvmOpts},
};
use foundry_config::{FsPermissions, FuzzConfig, InvariantConfig, RpcEndpoint, RpcEndpoints};
use napi::{
    bindgen_prelude::{BigInt, Buffer},
    Either, Status,
};
use napi_derive::napi;

use crate::cast::TryCast;

/// Default address of `tx.origin` in Foundry
///
/// `0x1804c8AB1F12E6bbf3894d4083f33e07309d1f38`
const DEFAULT_SENDER: Address = address!("1804c8AB1F12E6bbf3894d4083f33e07309d1f38");

/// Solidity test runner configuration arguments exposed through the ffi.
/// Docs based on https://book.getfoundry.sh/reference/config/testing
#[napi(object)]
#[derive(Clone)]
pub struct SolidityTestRunnerConfigArgs {
    /// The absolute path to the project root directory.
    /// Relative paths in cheat codes are resolved against this path.
    pub project_root: String,
    /// Configures the permissions of cheat codes that access the file system.
    pub fs_permissions: Option<Vec<PathPermission>>,
    /// Whether to collect traces. Defaults to false.
    pub trace: Option<bool>,
    /// Whether to collect debug info. Defaults to false.
    pub debug: Option<bool>,
    /// Address labels for traces. Defaults to none.
    pub labels: Option<Vec<AddressLabel>>,
    /// Whether to enable isolation of calls. In isolation mode all top-level
    /// calls are executed as a separate transaction in a separate EVM
    /// context, enabling more precise gas accounting and transaction state
    /// changes.
    /// Defaults to false.
    pub isolate: Option<bool>,
    /// Whether or not to enable the ffi cheatcode.
    /// Warning: Enabling this cheatcode has security implications, as it allows
    /// tests to execute arbitrary programs on your computer.
    /// Defaults to false.
    pub ffi: Option<bool>,
    /// The value of `msg.sender` in tests as hex string.
    /// Defaults to `0x1804c8AB1F12E6bbf3894d4083f33e07309d1f38`.
    pub sender: Option<Buffer>,
    /// The value of `tx.origin` in tests as hex string.
    /// Defaults to `0x1804c8AB1F12E6bbf3894d4083f33e07309d1f38`.
    pub tx_origin: Option<Buffer>,
    /// The initial balance of the sender in tests.
    /// Defaults to `0xffffffffffffffffffffffff`.
    pub initial_balance: Option<BigInt>,
    /// The value of `block.number` in tests.
    /// Defaults to `1`.
    pub block_number: Option<BigInt>,
    /// The value of the `chainid` opcode in tests.
    /// Defaults to `31337`.
    pub chain_id: Option<BigInt>,
    /// The gas limit for each test case.
    /// Defaults to `9_223_372_036_854_775_807` (`i64::MAX`).
    pub gas_limit: Option<BigInt>,
    /// The price of gas (in wei) in tests.
    /// Defaults to `0`.
    pub gas_price: Option<BigInt>,
    /// The base fee per gas (in wei) in tests.
    /// Defaults to `0`.
    pub block_base_fee_per_gas: Option<BigInt>,
    /// The value of `block.coinbase` in tests.
    /// Defaults to `0x0000000000000000000000000000000000000000`.
    pub block_coinbase: Option<Buffer>,
    /// The value of `block.timestamp` in tests.
    /// Defaults to 1.
    pub block_timestamp: Option<BigInt>,
    /// The value of `block.difficulty` in tests.
    /// Defaults to 0.
    pub block_difficulty: Option<BigInt>,
    /// The `block.gaslimit` value during EVM execution.
    /// Defaults to none.
    pub block_gas_limit: Option<BigInt>,
    /// Whether to disable the block gas limit.
    /// Defaults to false.
    pub disable_block_gas_limit: Option<bool>,
    /// The memory limit of the EVM in bytes.
    /// Defaults to 33_554_432 (2^25 = 32MiB).
    pub memory_limit: Option<BigInt>,
    /// If set, all tests are run in fork mode using this url or remote name.
    /// Defaults to none.
    pub eth_rpc_url: Option<String>,
    /// Pins the block number for the global state fork.
    pub fork_block_number: Option<BigInt>,
    /// Map of RPC endpoints from chain name to RPC urls for fork cheat codes,
    /// e.g. `{ "optimism": "https://optimism.alchemyapi.io/v2/..." }`
    pub rpc_endpoints: Option<HashMap<String, String>>,
    /// Optional RPC cache path. If this is none, then no RPC calls will be
    /// cached, otherwise data is cached to `<rpc_cache_path>/<chain
    /// id>/<block number>`. Caching can be disabled for specific chains
    /// with `rpc_storage_caching`.
    pub rpc_cache_path: Option<String>,
    /// What RPC endpoints are cached. Defaults to all.
    pub rpc_storage_caching: Option<StorageCachingConfig>,
    /// The number of seconds to wait before `vm.prompt` reverts with a timeout.
    /// Defaults to 120.
    pub prompt_timeout: Option<u32>,
    /// Fuzz testing configuration.
    pub fuzz: Option<FuzzConfigArgs>,
    /// Invariant testing configuration.
    /// If an invariant config setting is not set, but a corresponding fuzz
    /// config value is set, then the fuzz config value will be used.
    pub invariant: Option<InvariantConfigArgs>,
}

impl Debug for SolidityTestRunnerConfigArgs {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SolidityTestRunnerConfigArgs")
            .field("project_root", &self.project_root)
            .field("fs_permissions", &self.fs_permissions)
            .field("trace", &self.trace)
            .field("debug", &self.debug)
            .field("ffi", &self.ffi)
            .field("sender", &self.sender.as_ref().map(hex::encode))
            .field("tx_origin", &self.tx_origin.as_ref().map(hex::encode))
            .field("initial_balance", &self.initial_balance)
            .field("block_number", &self.block_number)
            .field("chain_id", &self.chain_id)
            .field("gas_limit", &self.gas_limit)
            .field("gas_price", &self.gas_price)
            .field("block_base_fee_per_gas", &self.block_base_fee_per_gas)
            .field(
                "block_coinbase",
                &self.block_coinbase.as_ref().map(hex::encode),
            )
            .field("block_timestamp", &self.block_timestamp)
            .field("block_difficulty", &self.block_difficulty)
            .field("block_gas_limit", &self.block_gas_limit)
            .field("memory_limit", &self.memory_limit)
            .field("eth_rpc_url", &self.eth_rpc_url)
            .field("rpc_cache_path", &self.rpc_cache_path)
            .field("rpc_endpoints", &self.rpc_endpoints)
            .field("rpc_storage_caching", &self.rpc_storage_caching)
            .field("prompt_timeout", &self.prompt_timeout)
            .field("fuzz", &self.fuzz)
            .field("invariant", &self.invariant)
            .finish()
    }
}

impl TryFrom<SolidityTestRunnerConfigArgs> for SolidityTestRunnerConfig {
    type Error = napi::Error;

    fn try_from(value: SolidityTestRunnerConfigArgs) -> Result<Self, Self::Error> {
        let SolidityTestRunnerConfigArgs {
            project_root,
            fs_permissions,
            trace,
            debug,
            labels,
            isolate,
            ffi,
            sender,
            tx_origin,
            initial_balance,
            block_number,
            chain_id,
            gas_limit,
            gas_price,
            block_base_fee_per_gas,
            block_coinbase,
            block_timestamp,
            block_difficulty,
            block_gas_limit,
            disable_block_gas_limit,
            memory_limit,
            eth_rpc_url,
            rpc_cache_path,
            fork_block_number,
            rpc_endpoints,
            rpc_storage_caching,
            prompt_timeout,
            fuzz,
            invariant,
        } = value;

        let invariant: InvariantConfig = fuzz
            .as_ref()
            .map(|f| invariant.unwrap_or_default().defaults_from_fuzz(f))
            .map(TryFrom::try_from)
            .transpose()?
            .unwrap_or_default();

        let fuzz: FuzzConfig = fuzz.map(TryFrom::try_from).transpose()?.unwrap_or_default();

        let cheats_config_options = CheatsConfigOptions {
            rpc_endpoints: rpc_endpoints
                .map(|endpoints| {
                    RpcEndpoints::new(
                        endpoints
                            .into_iter()
                            .map(|(chain, url)| (chain, RpcEndpoint::Url(url))),
                    )
                })
                .unwrap_or_default(),
            rpc_cache_path: rpc_cache_path.map(PathBuf::from),
            rpc_storage_caching: rpc_storage_caching
                .map(TryFrom::try_from)
                .transpose()?
                .unwrap_or_default(),
            unchecked_cheatcode_artifacts: false,
            fs_permissions: FsPermissions::new(
                fs_permissions
                    .unwrap_or_default()
                    .into_iter()
                    .map(Into::into),
            ),
            prompt_timeout: prompt_timeout.map_or(120, Into::into),
            labels: labels
                .unwrap_or_default()
                .into_iter()
                .map(|AddressLabel { address, label }| Ok((address.try_cast()?, label)))
                .collect::<Result<_, napi::Error>>()?,
        };

        let mut evm_opts = SolidityTestRunnerConfig::default_evm_opts();

        if let Some(gas_limit) = gas_limit {
            evm_opts.env.gas_limit = gas_limit.try_cast()?;
        }

        evm_opts.env.chain_id = chain_id.map(TryCast::try_cast).transpose()?;

        evm_opts.env.gas_price = gas_price.map(TryCast::try_cast).transpose()?;

        if let Some(block_base_fee_per_gas) = block_base_fee_per_gas {
            evm_opts.env.block_base_fee_per_gas = block_base_fee_per_gas.try_cast()?;
        }

        if let Some(tx_origin) = tx_origin {
            evm_opts.env.tx_origin = tx_origin.try_cast()?;
        }

        if let Some(block_number) = block_number {
            evm_opts.env.block_number = block_number.try_cast()?;
        }

        if let Some(block_difficulty) = block_difficulty {
            evm_opts.env.block_difficulty = block_difficulty.try_cast()?;
        }

        evm_opts.env.block_gas_limit = block_gas_limit.map(TryCast::try_cast).transpose()?;

        if let Some(block_timestamp) = block_timestamp {
            evm_opts.env.block_timestamp = block_timestamp.try_cast()?;
        }

        if let Some(block_coinbase) = block_coinbase {
            evm_opts.env.block_coinbase = block_coinbase.try_cast()?;
        }

        evm_opts.fork_url = eth_rpc_url;

        evm_opts.fork_block_number = fork_block_number.map(TryCast::try_cast).transpose()?;

        if let Some(isolate) = isolate {
            evm_opts.isolate = isolate;
        }

        if let Some(ffi) = ffi {
            evm_opts.ffi = ffi;
        }

        if let Some(sender) = sender {
            evm_opts.sender = sender.try_cast()?;
        }

        if let Some(initial_balance) = initial_balance {
            evm_opts.initial_balance = initial_balance.try_cast()?;
        }

        if let Some(memory_limit) = memory_limit {
            evm_opts.memory_limit = memory_limit.try_cast()?;
        }

        if let Some(disable_block_gas_limit) = disable_block_gas_limit {
            evm_opts.disable_block_gas_limit = disable_block_gas_limit;
        }

        Ok(SolidityTestRunnerConfig {
            project_root: project_root.into(),
            debug: debug.unwrap_or_default(),
            trace: trace.unwrap_or_default(),
            cheats_config_options,
            evm_opts,
            fuzz,
            invariant,
        })
    }
}

/// Fuzz testing configuration
#[napi(object)]
#[derive(Clone, Default, Debug)]
pub struct FuzzConfigArgs {
    /// Path where fuzz failures are recorded and replayed.
    pub failure_persist_dir: String,
    /// Name of the file to record fuzz failures, defaults to `failures`.
    pub failure_persist_file: Option<String>,
    /// The amount of fuzz runs to perform for each fuzz test case. Higher
    /// values gives more confidence in results at the cost of testing
    /// speed.
    /// Defaults to 256.
    pub runs: Option<u32>,
    /// The maximum number of combined inputs that may be rejected before the
    /// test as a whole aborts. “Global” filters apply to the whole test
    /// case. If the test case is rejected, the whole thing is regenerated.
    /// Defaults to 65536.
    pub max_test_rejects: Option<u32>,
    /// Hexadecimal string.
    /// Optional seed for the fuzzing RNG algorithm.
    /// Defaults to None.
    pub seed: Option<String>,
    /// Integer between 0 and 100.
    /// The weight of the dictionary. A higher dictionary weight will bias the
    /// fuzz inputs towards “interesting” values, e.g. boundary values like
    /// type(uint256).max or contract addresses from your environment.
    /// Defaults to 40.
    pub dictionary_weight: Option<u32>,
    /// The flag indicating whether to include values from storage.
    /// Defaults to true.
    pub include_storage: Option<bool>,
    /// The flag indicating whether to include push bytes values.
    /// Defaults to true.
    pub include_push_bytes: Option<bool>,
}

impl TryFrom<FuzzConfigArgs> for FuzzConfig {
    type Error = napi::Error;

    fn try_from(value: FuzzConfigArgs) -> Result<Self, Self::Error> {
        let FuzzConfigArgs {
            failure_persist_dir,
            failure_persist_file,
            runs,
            max_test_rejects,
            seed,
            dictionary_weight,
            include_storage,
            include_push_bytes,
        } = value;

        let failure_persist_dir = Some(failure_persist_dir.into());
        let failure_persist_file = Some(failure_persist_file.unwrap_or("failures".to_string()));
        let seed = seed
            .map(|s| {
                s.parse().map_err(|_err| {
                    napi::Error::new(Status::InvalidArg, format!("Invalid seed value: {s}"))
                })
            })
            .transpose()?;

        let mut fuzz = FuzzConfig {
            seed,
            failure_persist_dir,
            failure_persist_file,
            ..FuzzConfig::default()
        };

        if let Some(runs) = runs {
            fuzz.runs = runs;
        }

        if let Some(max_test_rejects) = max_test_rejects {
            fuzz.max_test_rejects = max_test_rejects;
        }

        if let Some(dictionary_weight) = dictionary_weight {
            fuzz.dictionary.dictionary_weight = dictionary_weight;
        }

        if let Some(include_storage) = include_storage {
            fuzz.dictionary.include_storage = include_storage;
        }

        if let Some(include_push_bytes) = include_push_bytes {
            fuzz.dictionary.include_push_bytes = include_push_bytes;
        }

        Ok(fuzz)
    }
}

/// Invariant testing configuration.
#[napi(object)]
#[derive(Clone, Default, Debug)]
pub struct InvariantConfigArgs {
    /// Path where invariant failures are recorded and replayed.
    pub failure_persist_dir: Option<String>,
    /// The number of runs that must execute for each invariant test group.
    /// Defaults to 256.
    pub runs: Option<u32>,
    /// The number of calls executed to attempt to break invariants in one run.
    /// Defaults to 500.
    pub depth: Option<u32>,
    /// Fails the invariant fuzzing if a revert occurs.
    /// Defaults to false.
    pub fail_on_revert: Option<bool>,
    /// Overrides unsafe external calls when running invariant tests, useful for
    /// e.g. performing reentrancy checks.
    /// Defaults to false.
    pub call_override: Option<bool>,
    /// Integer between 0 and 100.
    /// The weight of the dictionary. A higher dictionary weight will bias the
    /// fuzz inputs towards “interesting” values, e.g. boundary values like
    /// type(uint256).max or contract addresses from your environment.
    /// Defaults to 40.
    pub dictionary_weight: Option<u32>,
    /// The flag indicating whether to include values from storage.
    /// Defaults to true.
    pub include_storage: Option<bool>,
    /// The flag indicating whether to include push bytes values.
    /// Defaults to true.
    pub include_push_bytes: Option<bool>,
    /// The maximum number of attempts to shrink a failed the sequence. Shrink
    /// process is disabled if set to 0.
    /// Defaults to 5000.
    pub shrink_run_limit: Option<u32>,
}

impl InvariantConfigArgs {
    /// Fill in fields from the fuzz config if they are not set.
    fn defaults_from_fuzz(mut self, fuzz: &FuzzConfigArgs) -> Self {
        let FuzzConfigArgs {
            failure_persist_dir,
            runs,
            dictionary_weight,
            include_storage,
            include_push_bytes,
            // These aren't used in the invariant config.
            failure_persist_file: _,
            max_test_rejects: _,
            seed: _,
        } = fuzz;

        if self.failure_persist_dir.is_none() {
            self.failure_persist_dir = Some(failure_persist_dir.clone());
        }

        if self.runs.is_none() {
            self.runs = *runs;
        }

        if self.dictionary_weight.is_none() {
            self.dictionary_weight = *dictionary_weight;
        }

        if self.include_storage.is_none() {
            self.include_storage = *include_storage;
        }

        if self.include_push_bytes.is_none() {
            self.include_push_bytes = *include_push_bytes;
        }

        self
    }
}

impl From<InvariantConfigArgs> for InvariantConfig {
    fn from(value: InvariantConfigArgs) -> Self {
        let InvariantConfigArgs {
            failure_persist_dir,
            runs,
            depth,
            fail_on_revert,
            call_override,
            dictionary_weight,
            include_storage,
            include_push_bytes,
            shrink_run_limit,
        } = value;

        let failure_persist_dir = failure_persist_dir.map(PathBuf::from);

        let mut invariant = InvariantConfig {
            failure_persist_dir,
            ..InvariantConfig::default()
        };

        if let Some(runs) = runs {
            invariant.runs = runs;
        }

        if let Some(depth) = depth {
            invariant.depth = depth;
        }

        if let Some(fail_on_revert) = fail_on_revert {
            invariant.fail_on_revert = fail_on_revert;
        }

        if let Some(call_override) = call_override {
            invariant.call_override = call_override;
        }

        if let Some(dictionary_weight) = dictionary_weight {
            invariant.dictionary.dictionary_weight = dictionary_weight;
        }

        if let Some(include_storage) = include_storage {
            invariant.dictionary.include_storage = include_storage;
        }

        if let Some(include_push_bytes) = include_push_bytes {
            invariant.dictionary.include_push_bytes = include_push_bytes;
        }

        if let Some(shrink_run_limit) = shrink_run_limit {
            invariant.shrink_run_limit = shrink_run_limit.try_into().expect("u32 fits into usize");
        }

        invariant
    }
}

/// Settings to configure caching of remote
#[napi(object)]
#[derive(Clone, Debug)]
pub struct StorageCachingConfig {
    /// Chains to cache. Either all or none or a list of chain names, e.g.
    /// ["optimism", "mainnet"].
    pub chains: Either<CachedChains, Vec<String>>,
    /// Endpoints to cache. Either all or remote or a regex.
    pub endpoints: Either<CachedEndpoints, String>,
}

impl Default for StorageCachingConfig {
    fn default() -> Self {
        Self {
            chains: Either::A(CachedChains::default()),
            endpoints: Either::A(CachedEndpoints::default()),
        }
    }
}

impl TryFrom<StorageCachingConfig> for foundry_config::cache::StorageCachingConfig {
    type Error = napi::Error;

    fn try_from(value: StorageCachingConfig) -> Result<Self, Self::Error> {
        let chains = match value.chains {
            Either::A(chains) => chains.into(),
            Either::B(chains) => {
                let chains = chains
                    .into_iter()
                    .map(|c| {
                        c.parse()
                            .map_err(|c| napi::Error::new(Status::InvalidArg, c))
                    })
                    .collect::<Result<_, _>>()?;
                foundry_config::cache::CachedChains::Chains(chains)
            }
        };
        let endpoints = match value.endpoints {
            Either::A(endpoints) => endpoints.into(),
            Either::B(regex) => {
                let regex = regex.parse().map_err(|_err| {
                    napi::Error::new(Status::InvalidArg, format!("Invalid regex: {regex}"))
                })?;
                foundry_config::cache::CachedEndpoints::Pattern(regex)
            }
        };
        Ok(Self { chains, endpoints })
    }
}

/// What chains to cache
#[napi]
#[derive(Debug, Default)]
pub enum CachedChains {
    /// Cache all chains
    #[default]
    All,
    /// Don't cache anything
    None,
}

impl From<CachedChains> for foundry_config::cache::CachedChains {
    fn from(value: CachedChains) -> Self {
        match value {
            CachedChains::All => foundry_config::cache::CachedChains::All,
            CachedChains::None => foundry_config::cache::CachedChains::None,
        }
    }
}

/// What endpoints to enable caching for
#[napi]
#[derive(Debug, Default)]
pub enum CachedEndpoints {
    /// Cache all endpoints
    #[default]
    All,
    /// Only cache non-local host endpoints
    Remote,
}

impl From<CachedEndpoints> for foundry_config::cache::CachedEndpoints {
    fn from(value: CachedEndpoints) -> Self {
        match value {
            CachedEndpoints::All => foundry_config::cache::CachedEndpoints::All,
            CachedEndpoints::Remote => foundry_config::cache::CachedEndpoints::Remote,
        }
    }
}

/// Represents an access permission to a single path
#[napi(object)]
#[derive(Clone, Debug)]
pub struct PathPermission {
    /// Permission level to access the `path`
    pub access: FsAccessPermission,
    /// The targeted path guarded by the permission
    pub path: String,
}

impl From<PathPermission> for foundry_config::fs_permissions::PathPermission {
    fn from(value: PathPermission) -> Self {
        let PathPermission { access, path } = value;
        Self {
            access: access.into(),
            path: path.into(),
        }
    }
}

/// Determines the status of file system access
#[napi]
#[derive(Debug)]
pub enum FsAccessPermission {
    /// FS access is allowed with `read` + `write` permission
    ReadWrite,
    /// Only reading is allowed
    Read,
    /// Only writing is allowed
    Write,
}

impl From<FsAccessPermission> for foundry_config::fs_permissions::FsAccessPermission {
    fn from(value: FsAccessPermission) -> Self {
        match value {
            FsAccessPermission::ReadWrite => {
                foundry_config::fs_permissions::FsAccessPermission::ReadWrite
            }
            FsAccessPermission::Read => foundry_config::fs_permissions::FsAccessPermission::Read,
            FsAccessPermission::Write => foundry_config::fs_permissions::FsAccessPermission::Write,
        }
    }
}

#[napi(object)]
#[derive(Clone)]
pub struct AddressLabel {
    /// The address to label
    pub address: Buffer,
    /// The label to assign to the address
    pub label: String,
}

impl Debug for AddressLabel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AddressLabel")
            .field("address", &hex::encode(&self.address))
            .field("label", &self.label)
            .finish()
    }
}

/// Solidity tests configuration
#[derive(Clone, Debug)]
pub(super) struct SolidityTestRunnerConfig {
    /// Project root directory.
    pub project_root: PathBuf,
    /// Whether to enable debug mode.
    pub debug: bool,
    /// Whether to enable trace mode.
    pub trace: bool,
    /// Cheats configuration options
    pub cheats_config_options: CheatsConfigOptions,
    /// EVM options
    pub evm_opts: EvmOpts,
    /// Configuration for fuzz testing
    pub fuzz: FuzzConfig,
    /// Configuration for invariant testing
    pub invariant: InvariantConfig,
}

impl SolidityTestRunnerConfig {
    /// The default evm options for the Solidity test runner.
    pub fn default_evm_opts() -> EvmOpts {
        EvmOpts {
            env: EvmEnv {
                gas_limit: i64::MAX.try_into().expect("max i64 fits into u64"),
                chain_id: Some(31337),
                gas_price: Some(0),
                block_base_fee_per_gas: 0,
                tx_origin: DEFAULT_SENDER,
                block_number: 1,
                block_difficulty: 0,
                block_prevrandao: B256::default(),
                block_gas_limit: None,
                block_timestamp: 1,
                block_coinbase: Address::default(),
                code_size_limit: None,
            },
            fork_url: None,
            fork_block_number: None,
            fork_retries: None,
            fork_retry_backoff: None,
            compute_units_per_second: None,
            no_rpc_rate_limit: false,
            sender: DEFAULT_SENDER,
            initial_balance: U256::from(0xffffffffffffffffffffffffu128),
            ffi: false,
            always_use_create_2_factory: false,
            memory_limit: 1 << 25, // 2**25 = 32MiB
            isolate: false,
            disable_block_gas_limit: false,
        }
    }

    pub async fn get_fork(&self) -> Result<Option<CreateFork>, napi::Error> {
        if let Some(fork_url) = self.evm_opts.fork_url.as_ref() {
            let evm_env = self
                .evm_opts
                .fork_evm_env(fork_url)
                .await
                .map_err(|e| {
                    napi::Error::new(
                        Status::GenericFailure,
                        format!("Failed to get fork config: {e:?}"),
                    )
                })?
                .0;

            let rpc_cache_path = self.rpc_cache_path(fork_url, evm_env.cfg.chain_id);

            Ok(Some(CreateFork {
                rpc_cache_path,
                url: fork_url.clone(),
                env: evm_env,
                evm_opts: self.evm_opts.clone(),
            }))
        } else {
            Ok(None)
        }
    }

    /// Whether caching should be enabled for the given chain id
    fn rpc_cache_path(&self, endpoint: &str, chain_id: impl Into<u64>) -> Option<PathBuf> {
        let enable_for_chain_id = self
            .cheats_config_options
            .rpc_storage_caching
            .enable_for_chain_id(chain_id.into());
        let enable_for_endpoint = self
            .cheats_config_options
            .rpc_storage_caching
            .enable_for_endpoint(endpoint);
        if enable_for_chain_id && enable_for_endpoint {
            self.cheats_config_options.rpc_cache_path.clone()
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use foundry_config::NamedChain;

    use super::*;

    impl SolidityTestRunnerConfig {
        fn new_for_caching_tests(project_root: PathBuf) -> Self {
            let cheats_config_options = CheatsConfigOptions {
                rpc_endpoints: RpcEndpoints::default(),
                rpc_cache_path: Some(project_root.join("rpc-cache")),
                rpc_storage_caching: foundry_config::cache::StorageCachingConfig::default(),
                unchecked_cheatcode_artifacts: false,
                fs_permissions: FsPermissions::default(),
                prompt_timeout: 120,
                labels: HashMap::default(),
            };
            Self {
                project_root,
                debug: false,
                trace: false,
                cheats_config_options,
                evm_opts: Self::default_evm_opts(),
                fuzz: FuzzConfig::default(),
                invariant: InvariantConfig::default(),
            }
        }
    }

    #[test]
    fn test_rpc_cache_path() {
        let mut config = SolidityTestRunnerConfig::new_for_caching_tests(
            "/tmp/fake-path".parse().expect("path ok"),
        );

        let url = "https://eth-mainnet.alchemyapi";
        assert_eq!(
            config.rpc_cache_path(url, NamedChain::Mainnet),
            Some("/tmp/fake-path/rpc-cache".parse().expect("path ok"))
        );
        assert!(config.rpc_cache_path(url, NamedChain::Dev).is_none());

        config.cheats_config_options.rpc_storage_caching.chains =
            foundry_config::cache::CachedChains::None;
        assert!(config.rpc_cache_path(url, NamedChain::Mainnet).is_none());

        config.cheats_config_options.rpc_storage_caching.chains =
            foundry_config::cache::CachedChains::All;
        assert!(config.rpc_cache_path(url, NamedChain::Mainnet).is_some());
        config.cheats_config_options.rpc_storage_caching.endpoints =
            foundry_config::cache::CachedEndpoints::Pattern("sepolia".parse().expect("regex ok"));
        assert!(config.rpc_cache_path(url, NamedChain::Mainnet).is_none());

        config.cheats_config_options.rpc_storage_caching.endpoints =
            foundry_config::cache::CachedEndpoints::All;
        assert!(config.rpc_cache_path(url, NamedChain::Mainnet).is_some());
        config.cheats_config_options.rpc_cache_path = None;
        assert!(config.rpc_cache_path(url, NamedChain::Mainnet).is_none());
    }
}
