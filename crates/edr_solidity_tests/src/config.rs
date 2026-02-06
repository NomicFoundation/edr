use std::{collections::HashMap, path::PathBuf};

pub use edr_coverage::reporter::SyncOnCollectedCoverageCallback;
use edr_primitives::{Address, B256, U256};
use edr_solidity::config::IncludeTraces;
use foundry_cheatcodes::TestFunctionIdentifier;
use foundry_evm::{
    backend::Predeploy,
    evm_context::{BlockEnvTr, HardforkTr, TransactionEnvTr},
    fuzz::{invariant::InvariantConfig, FuzzConfig},
    inspectors::cheatcodes::CheatsConfigOptions,
};

use crate::{
    fork::CreateFork,
    opts::{Env as EvmEnv, EvmOpts},
};

#[derive(Debug, thiserror::Error)]
pub enum SolidityTestRunnerConfigError {
    /// Failed to create a fork with the given config.
    #[error("{0}")]
    CreateFork(eyre::Error),
    /// Failed to create an EVM environment with the given config.
    #[error("{0}")]
    EvmEnv(eyre::Error),
    /// Failed to normalize project root
    #[error("Failed to normalize project root with error: {0}")]
    InvalidProjectRoot(std::io::Error),
}

/// Solidity tests configuration
#[derive(Clone)]
pub struct SolidityTestRunnerConfig<HardforkT: HardforkTr> {
    /// Project root directory.
    pub project_root: PathBuf,
    /// Whether to collect stack traces.
    pub collect_stack_traces: CollectStackTraces,
    /// Whether to enable trace mode and which traces to include in test
    /// results.
    pub include_traces: IncludeTraces,
    /// Whether to collect coverage info
    pub coverage: bool,
    /// Whether to enable solidity fuzz fixtures support
    pub enable_fuzz_fixtures: bool,
    /// Whether to enable table test support
    pub enable_table_tests: bool,
    /// Cheats configuration options
    pub cheats_config_options: CheatsConfigOptions,
    /// EVM options
    pub evm_opts: EvmOpts<HardforkT>,
    /// The predeploys applied in local mode.
    /// These should match the predeploys of the network in fork mode, so they
    /// aren't set in fork mode.
    pub local_predeploys: Vec<Predeploy>,
    /// Configuration for fuzz testing
    pub fuzz: FuzzConfig,
    /// Configuration for invariant testing
    pub invariant: InvariantConfig,
    /// Optionally, a callback for reporting collected coverage hits.
    pub on_collected_coverage_fn: Option<Box<dyn SyncOnCollectedCoverageCallback>>,
    /// Whether to generate a gas report after running tests
    pub generate_gas_report: bool,
    /// Test function level config overrides.
    pub test_function_overrides: HashMap<TestFunctionIdentifier, TestFunctionConfigOverride>,
}

impl<HardforkT: HardforkTr> SolidityTestRunnerConfig<HardforkT> {
    pub async fn get_fork<BlockT: BlockEnvTr, TransactionT: TransactionEnvTr>(
        &self,
    ) -> Result<Option<CreateFork<BlockT, TransactionT, HardforkT>>, SolidityTestRunnerConfigError>
    {
        if let Some(fork_url) = self.evm_opts.fork_url.as_ref() {
            let evm_env = self
                .evm_opts
                .fork_evm_env(fork_url)
                .await
                .map_err(SolidityTestRunnerConfigError::CreateFork)?
                .0;

            let rpc_cache_path = self.rpc_cache_path(fork_url, evm_env.cfg.chain_id);

            Ok(Some(CreateFork::<BlockT, TransactionT, HardforkT> {
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

impl<HardforkT: HardforkTr> SolidityTestRunnerConfig<HardforkT> {
    /// The default evm options for the Solidity test runner.
    pub fn default_evm_opts() -> EvmOpts<HardforkT> {
        EvmOpts {
            env: EvmEnv {
                gas_limit: i64::MAX.try_into().expect("max i64 fits into u64"),
                chain_id: Some(31337),
                gas_price: Some(0),
                block_base_fee_per_gas: 0,
                tx_origin: edr_defaults::SOLIDITY_TESTS_SENDER,
                block_number: U256::from(1),
                block_difficulty: 0,
                block_prevrandao: B256::default(),
                block_gas_limit: None,
                block_timestamp: U256::from(1),
                block_coinbase: Address::default(),
                code_size_limit: None,
            },
            spec: HardforkT::default(),
            fork_url: None,
            fork_block_number: None,
            fork_retries: None,
            fork_retry_backoff: None,
            compute_units_per_second: None,
            no_rpc_rate_limit: false,
            sender: edr_defaults::SOLIDITY_TESTS_SENDER,
            initial_balance: U256::from(0xffffffffffffffffffffffffu128),
            ffi: false,
            memory_limit: 1 << 25, // 2**25 = 32MiB
            isolate: false,
            disable_block_gas_limit: false,
            fork_headers: None,
        }
    }
}

/// A type that controls when stack traces are collected.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CollectStackTraces {
    /// Always collects stack traces, adding performance overhead.
    Always,
    /// Only collects stack traces upon failure, re-executing the test. This
    /// minimizes performance overhead.
    ///
    /// Not all tests can be re-executed since certain cheatcodes contain
    /// non-deterministic side-effects.
    OnFailure,
}

/// Test function level config override.
#[derive(Clone, Debug)]
pub struct TestFunctionConfigOverride {
    /// Allow expecting reverts with `expectRevert` at the same callstack depth
    /// as the test.
    pub allow_internal_expect_revert: Option<bool>,
    /// Configuration override for fuzz testing
    pub fuzz: Option<FuzzConfigOverride>,
    /// Configuration override for invariant testing
    pub invariant: Option<InvariantConfigOverride>,
}

/// Timeout configuration.
#[derive(Clone, Copy, Debug, Default)]
pub struct TimeoutConfig {
    /// Optional timeout (in seconds)
    pub time: Option<u32>,
}

/// Test function or test contract level fuzz config override.
#[derive(Clone, Debug, Default)]
pub struct FuzzConfigOverride {
    /// The number of test cases that must execute for each property test
    pub runs: Option<u32>,
    /// The maximum number of test case rejections allowed by proptest, to be
    /// encountered during usage of `vm.assume` cheatcode. This will be used
    /// to set the `max_global_rejects` value in proptest test runner config.
    /// `max_local_rejects` option isn't exposed here since we're not using
    /// `prop_filter`.
    pub max_test_rejects: Option<u32>,
    /// Show `console.log` in fuzz test.
    pub show_logs: Option<bool>,
    /// Optional timeout (in seconds) for each property test
    pub timeout: Option<TimeoutConfig>,
}

/// Test function or test contract level invariant config override.
#[derive(Clone, Debug, Default)]
pub struct InvariantConfigOverride {
    /// The number of runs that must execute for each invariant test group.
    pub runs: Option<u32>,
    /// The number of calls executed to attempt to break invariants in one run.
    pub depth: Option<u32>,
    /// Fails the invariant fuzzing if a revert occurs
    pub fail_on_revert: Option<bool>,
    /// Allows overriding an unsafe external call when running invariant tests.
    /// eg. reentrancy checks
    pub call_override: Option<bool>,
    /// Optional timeout (in seconds) for each invariant test.
    pub timeout: Option<TimeoutConfig>,
}
