//! Test helpers for Forge integration tests.

mod config;
pub use config::{assert_multiple, TestConfig};
mod integration_test_config;
mod solidity_error_code;
mod solidity_test_filter;
use edr_eth::{
    l1::{self, BlockEnv},
    spec::HaltReasonTrait,
};
pub use solidity_test_filter::SolidityTestFilter;
mod tracing;

use std::{borrow::Cow, env, fmt, io::Write, marker::PhantomData, path::PathBuf};

use alloy_primitives::{Bytes, U256};
use edr_solidity::{
    artifacts::ArtifactId,
    linker::{LinkOutput, Linker},
};
use edr_solidity_tests::{
    fuzz::FuzzDictionaryConfig,
    multi_runner::{TestContract, TestContracts},
    revm::context::TxEnv,
    IncludeTraces, MultiContractRunner, SolidityTestRunnerConfig,
};
use edr_test_utils::{
    env::{get_alchemy_url_for_network, NetworkType},
    new_fd_lock,
};
use foundry_cheatcodes::{ExecutionContextConfig, FsPermissions, RpcEndpoint, RpcEndpoints};
use foundry_compilers::{
    artifacts::{CompactContractBytecode, CompactContractBytecodeCow, EvmVersion, Libraries},
    Artifact, Project, ProjectCompileOutput,
};
use foundry_evm::{
    abi::TestFunctionExt,
    constants::{CALLER, LIBRARY_DEPLOYER},
    contracts::ContractsByArtifact,
    decode::RevertDecoder,
    evm_context::{
        BlockEnvTr, ChainContextTr, EvmBuilderTrait, HardforkTr, L1EvmBuilder, TransactionEnvTr,
        TransactionErrorTrait,
    },
    executors::invariant::InvariantConfig,
    fuzz::FuzzConfig,
    inspectors::cheatcodes::CheatsConfigOptions,
    opts::{Env, EvmOpts},
};
use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};

use crate::helpers::{
    config::NoOpContractDecoder, integration_test_config::IntegrationTestConfig,
    tracing::init_tracing_for_solidity_tests,
};

pub const RE_PATH_SEPARATOR: &str = "/";
static PROJECT_ROOT: Lazy<PathBuf> = Lazy::new(|| {
    const TESTDATA: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/testdata");
    dunce::canonicalize(PathBuf::from(TESTDATA)).expect("Failed to canonicalize root")
});

/// Profile for the tests group. Used to configure separate configurations for
/// test runs.
pub enum ForgeTestProfile {
    Default,
    Cancun,
    MultiVersion,
}

impl fmt::Display for ForgeTestProfile {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ForgeTestProfile::Default => write!(f, "default"),
            ForgeTestProfile::Cancun => write!(f, "cancun"),
            ForgeTestProfile::MultiVersion => write!(f, "multi-version"),
        }
    }
}

impl ForgeTestProfile {
    /// Returns true if the profile is Cancun.
    fn is_cancun(&self) -> bool {
        matches!(self, Self::Cancun)
    }

    fn project(&self) -> Project {
        self.integration_test_config()
            .project()
            .expect("Failed to build project")
    }

    fn evm_opts<HardforkT: HardforkTr>(hardfork: HardforkT) -> EvmOpts<HardforkT> {
        EvmOpts {
            env: Env {
                gas_limit: u64::MAX,
                chain_id: None,
                tx_origin: CALLER,
                block_number: 1,
                block_timestamp: 1,
                ..Env::default()
            },
            sender: CALLER,
            initial_balance: U256::MAX,
            ffi: true,
            memory_limit: 1 << 26,
            spec: hardfork,
            ..EvmOpts::default()
        }
    }

    fn runner_config<HardforkT: HardforkTr>(
        hardfork: HardforkT,
    ) -> SolidityTestRunnerConfig<HardforkT> {
        SolidityTestRunnerConfig {
            include_traces: IncludeTraces::All,
            evm_opts: Self::evm_opts(hardfork),
            project_root: PROJECT_ROOT.clone(),
            cheats_config_options: CheatsConfigOptions {
                execution_context: ExecutionContextConfig::Test,
                ..CheatsConfigOptions::default()
            },
            fuzz: TestFuzzConfig::default().into(),
            invariant: TestInvariantConfig::default().into(),
            coverage: false,
            test_fail: true,
            solidity_fuzz_fixtures: true,
            local_predeploys: Vec::default(),
            on_collected_coverage_fn: None,
        }
    }

    /// Build [`IntegrationTestConfig`] for test profile.
    ///
    /// Project source files are read from `testdata/{profile_name`}
    /// Project output files are written to `testdata/out/{profile_name`}
    /// Cache is written to `testdata/cache/{profile_name`}
    ///
    /// AST output is enabled by default to support inline configs.
    fn integration_test_config(&self) -> IntegrationTestConfig {
        let mut config = IntegrationTestConfig::with_root(PROJECT_ROOT.clone());

        config.ast = true;
        config.src = PROJECT_ROOT.join(self.to_string());
        config.out = PROJECT_ROOT.join("out").join(self.to_string());
        config.cache_path = PROJECT_ROOT.join("cache").join(self.to_string());
        config.libraries = vec![
            "fork/Fork.t.sol:DssExecLib:0xfD88CeE74f7D78697775aBDAE53f9Da1559728E4".to_string(),
        ];

        if self.is_cancun() {
            config.evm_version = EvmVersion::Cancun;
        }

        config
    }
}

/// Fuzz testing config with different defaults than
/// [`foundry_config::FuzzConfig`]. See [`foundry_config::FuzzConfig`] for
/// documentation.
#[derive(Debug, Clone)]
pub struct TestFuzzConfig {
    pub runs: u32,
    pub max_test_rejects: u32,
    pub seed: Option<U256>,
    pub dictionary: TestFuzzDictionaryConfig,
    pub gas_report_samples: u32,
    pub failure_persist_dir: Option<PathBuf>,
    pub failure_persist_file: String,
}

impl Default for TestFuzzConfig {
    fn default() -> Self {
        TestFuzzConfig {
            runs: 256,
            max_test_rejects: 65536,
            seed: None,
            dictionary: TestFuzzDictionaryConfig::default(),
            gas_report_samples: 256,
            failure_persist_dir: Some(tempfile::tempdir().unwrap().into_path()),
            failure_persist_file: "testfailure".into(),
        }
    }
}

impl From<TestFuzzConfig> for FuzzConfig {
    fn from(value: TestFuzzConfig) -> Self {
        FuzzConfig {
            runs: value.runs,
            max_test_rejects: value.max_test_rejects,
            seed: value.seed,
            dictionary: value.dictionary.into(),
            gas_report_samples: value.gas_report_samples,
            failure_persist_dir: value.failure_persist_dir,
            failure_persist_file: value.failure_persist_file,
            show_logs: false,
            timeout: None,
        }
    }
}

/// Fuzz testing config with different defaults than
/// [`foundry_config::InvariantConfig`]. See [`foundry_config::InvariantConfig`]
/// for documentation.
#[derive(Debug, Clone)]
pub struct TestInvariantConfig {
    pub runs: u32,
    pub depth: u32,
    pub fail_on_revert: bool,
    pub call_override: bool,
    pub dictionary: FuzzDictionaryConfig,
    pub shrink_run_limit: u32,
    pub max_assume_rejects: u32,
    pub gas_report_samples: u32,
    pub failure_persist_dir: Option<PathBuf>,
}

impl Default for TestInvariantConfig {
    fn default() -> Self {
        TestInvariantConfig {
            runs: 256,
            depth: 15,
            fail_on_revert: false,
            call_override: false,
            dictionary: FuzzDictionaryConfig {
                dictionary_weight: 80,
                include_storage: true,
                include_push_bytes: true,
                max_fuzz_dictionary_addresses: 10_000,
                max_fuzz_dictionary_values: 10_000,
            },
            shrink_run_limit: 2_u32.pow(18u32),
            max_assume_rejects: 65536,
            gas_report_samples: 256,
            failure_persist_dir: Some(tempfile::tempdir().unwrap().into_path()),
        }
    }
}

impl From<TestInvariantConfig> for InvariantConfig {
    fn from(value: TestInvariantConfig) -> Self {
        InvariantConfig {
            runs: value.runs,
            depth: value.depth,
            fail_on_revert: value.fail_on_revert,
            call_override: value.call_override,
            dictionary: value.dictionary,
            shrink_run_limit: value.shrink_run_limit,
            max_assume_rejects: value.max_assume_rejects,
            gas_report_samples: value.gas_report_samples,
            failure_persist_dir: value.failure_persist_dir,
            show_metrics: false,
            timeout: None,
            show_solidity: false,
        }
    }
}

/// Fuzz dictionary config with different defaults than
/// [`foundry_config::FuzzDictionaryConfig`].
/// See [`foundry_config::FuzzDictionaryConfig`] for documentation.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TestFuzzDictionaryConfig {
    pub dictionary_weight: u32,
    pub include_storage: bool,
    pub include_push_bytes: bool,
    pub max_fuzz_dictionary_addresses: usize,
    pub max_fuzz_dictionary_values: usize,
}

impl Default for TestFuzzDictionaryConfig {
    fn default() -> Self {
        TestFuzzDictionaryConfig {
            dictionary_weight: 40,
            include_storage: true,
            include_push_bytes: true,
            max_fuzz_dictionary_addresses: 10_000,
            max_fuzz_dictionary_values: 10_000,
        }
    }
}

impl From<TestFuzzDictionaryConfig> for FuzzDictionaryConfig {
    fn from(value: TestFuzzDictionaryConfig) -> Self {
        FuzzDictionaryConfig {
            dictionary_weight: value.dictionary_weight,
            include_storage: value.include_storage,
            include_push_bytes: value.include_push_bytes,
            max_fuzz_dictionary_addresses: value.max_fuzz_dictionary_addresses,
            max_fuzz_dictionary_values: value.max_fuzz_dictionary_values,
        }
    }
}

/// Type alias for [`ForgeTestData`] targetting L1.
pub type L1ForgeTestData = ForgeTestData<
    BlockEnv,
    (),
    L1EvmBuilder,
    l1::HaltReason,
    l1::SpecId,
    l1::InvalidTransaction,
    TxEnv,
>;

/// Container for test data for a specific test profile.
pub struct ForgeTestData<
    BlockT: BlockEnvTr,
    ChainContextT: ChainContextTr,
    EvmBuilderT: EvmBuilderTrait<BlockT, ChainContextT, HaltReasonT, HardforkT, TransactionErrorT, TransactionT>,
    HaltReasonT: HaltReasonTrait,
    HardforkT: HardforkTr,
    TransactionErrorT: TransactionErrorTrait,
    TransactionT: TransactionEnvTr,
> {
    project: Project,
    test_contracts: TestContracts,
    known_contracts: ContractsByArtifact,
    libs_to_deploy: Vec<Bytes>,
    revert_decoder: RevertDecoder,
    hardfork: HardforkT,
    #[allow(clippy::type_complexity)]
    _phantom: PhantomData<
        fn() -> (
            BlockT,
            ChainContextT,
            EvmBuilderT,
            HaltReasonT,
            TransactionErrorT,
            TransactionT,
        ),
    >,
}

impl<
        BlockT: BlockEnvTr,
        ChainContextT: ChainContextTr,
        EvmBuilderT: EvmBuilderTrait<
            BlockT,
            ChainContextT,
            HaltReasonT,
            HardforkT,
            TransactionErrorT,
            TransactionT,
        >,
        HaltReasonT: 'static + HaltReasonTrait + TryInto<l1::HaltReason> + Send + Sync,
        HardforkT: HardforkTr,
        TransactionErrorT: TransactionErrorTrait,
        TransactionT: TransactionEnvTr,
    >
    ForgeTestData<
        BlockT,
        ChainContextT,
        EvmBuilderT,
        HaltReasonT,
        HardforkT,
        TransactionErrorT,
        TransactionT,
    >
{
    /// Builds [`ForgeTestData`] for the given [`ForgeTestProfile`].
    ///
    /// Uses [`get_compiled`] to lazily compile the project.
    pub fn new(profile: ForgeTestProfile, hardfork: HardforkT) -> eyre::Result<Self> {
        let project = profile.project();
        let output = get_compiled(&project);

        let root = project.root();
        let contracts = output
            .clone()
            .with_stripped_file_prefixes(root)
            .into_artifacts()
            .map(|(id, contract)| {
                let id = ArtifactId {
                    name: id.name,
                    source: id.source,
                    version: id.version,
                };
                let CompactContractBytecode {
                    abi,
                    bytecode,
                    deployed_bytecode,
                } = contract.into_contract_bytecode();
                let contract_cow = CompactContractBytecodeCow {
                    abi: abi.map(Cow::Owned),
                    bytecode: bytecode.map(Cow::Owned),
                    deployed_bytecode: deployed_bytecode.map(Cow::Owned),
                };
                (id, contract_cow)
            });
        let linker = Linker::new(root, contracts);

        // Build revert decoder from ABIs of all artifacts.
        let abis = linker
            .contracts
            .iter()
            .filter_map(|(_, contract)| contract.abi.as_ref().map(std::borrow::Borrow::borrow));
        let revert_decoder = RevertDecoder::new().with_abis(abis);

        let LinkOutput {
            libraries,
            libs_to_deploy,
        } = linker.link_with_nonce_or_address(
            Libraries::default(),
            LIBRARY_DEPLOYER,
            0,
            linker.contracts.keys(),
        )?;

        let linked_contracts = linker.get_linked_artifacts(&libraries)?;

        // Create a mapping of name => (abi, deployment code, Vec<library deployment
        // code>)
        let mut test_contracts = TestContracts::default();

        for (id, contract) in linked_contracts.iter() {
            let Some(abi) = contract.abi.as_ref() else {
                continue;
            };

            // if it's a test, link it and add to deployable contracts
            if abi.constructor.as_ref().is_none_or(|c| c.inputs.is_empty())
                && abi.functions().any(|func| func.name.is_any_test())
            {
                let Some(bytecode) = contract
                    .get_bytecode_bytes()
                    .map(Cow::into_owned)
                    .filter(|b| !b.is_empty())
                else {
                    continue;
                };

                test_contracts.insert(
                    id.clone(),
                    TestContract {
                        abi: abi.clone().into_owned(),
                        bytecode,
                    },
                );
            }
        }

        let known_contracts = ContractsByArtifact::new(linked_contracts);

        Ok(Self {
            project,
            test_contracts,
            known_contracts,
            libs_to_deploy,
            revert_decoder,
            hardfork,
            _phantom: PhantomData,
        })
    }

    /// Builds a [`SolidityTestRunnerConfig`] with mock RPC endpoints.
    pub fn config_with_mock_rpc(&self) -> SolidityTestRunnerConfig<HardforkT> {
        init_tracing_for_solidity_tests();
        // Construct a new one to create new failure persistance directory for each test
        let mut config = ForgeTestProfile::runner_config(self.hardfork);
        config.cheats_config_options.rpc_endpoints = mock_rpc_endpoints();

        config
    }

    /// Builds a [`SolidityTestRunnerConfig`] with remote RPC endpoints and RPC
    /// cache path.
    pub fn config_with_remote_rpc(&self) -> SolidityTestRunnerConfig<HardforkT> {
        init_tracing_for_solidity_tests();
        // Construct a new one to create new failure persistance directory for each test
        let mut config = ForgeTestProfile::runner_config(self.hardfork);
        config.cheats_config_options.rpc_endpoints = remote_rpc_endpoints();
        //`**/edr-cache` is cached in CI
        config.cheats_config_options.rpc_cache_path =
            Some(self.project.root().join("edr-cache/solidity-tests/rpc"));
        config
    }

    /// Builds a non-tracing runner
    pub async fn runner(
        &self,
    ) -> MultiContractRunner<
        BlockT,
        ChainContextT,
        EvmBuilderT,
        HaltReasonT,
        HardforkT,
        NoOpContractDecoder<HaltReasonT>,
        TransactionErrorT,
        TransactionT,
    > {
        let config = self.config_with_mock_rpc();
        self.runner_with_config(config).await
    }

    /// Builds a non-tracing runner with the given config
    pub async fn runner_with_config(
        &self,
        mut config: SolidityTestRunnerConfig<HardforkT>,
    ) -> MultiContractRunner<
        BlockT,
        ChainContextT,
        EvmBuilderT,
        HaltReasonT,
        HardforkT,
        NoOpContractDecoder<HaltReasonT>,
        TransactionErrorT,
        TransactionT,
    > {
        // no prompt testing
        config.cheats_config_options.prompt_timeout = 0;

        self.build_runner(config).await
    }

    /// Builds a non-tracing runner with the given filesystem permissions
    pub async fn runner_with_fs_permissions(
        &self,
        fs_permissions: FsPermissions,
        mut config: SolidityTestRunnerConfig<HardforkT>,
    ) -> MultiContractRunner<
        BlockT,
        ChainContextT,
        EvmBuilderT,
        HaltReasonT,
        HardforkT,
        NoOpContractDecoder<HaltReasonT>,
        TransactionErrorT,
        TransactionT,
    > {
        config.cheats_config_options.fs_permissions = fs_permissions;
        self.runner_with_config(config).await
    }

    /// Builds a non-tracing runner with the given invariant config
    pub async fn runner_with_fuzz_config(
        &self,
        fuzz_config: TestFuzzConfig,
    ) -> MultiContractRunner<
        BlockT,
        ChainContextT,
        EvmBuilderT,
        HaltReasonT,
        HardforkT,
        NoOpContractDecoder<HaltReasonT>,
        TransactionErrorT,
        TransactionT,
    > {
        let mut config = self.config_with_mock_rpc();
        config.fuzz = fuzz_config.into();
        self.runner_with_config(config).await
    }

    /// Builds a non-tracing runner with the given invariant config
    pub async fn runner_with_invariant_config(
        &self,
        invariant_config: TestInvariantConfig,
    ) -> MultiContractRunner<
        BlockT,
        ChainContextT,
        EvmBuilderT,
        HaltReasonT,
        HardforkT,
        NoOpContractDecoder<HaltReasonT>,
        TransactionErrorT,
        TransactionT,
    > {
        let mut config = self.config_with_mock_rpc();
        config.invariant = invariant_config.into();
        self.runner_with_config(config).await
    }

    /// Builds a non-tracing runner with the given invariant config and fuzz
    /// seed.
    pub async fn runner_with_invariant_config_and_seed(
        &self,
        seed: U256,
        invariant_config: TestInvariantConfig,
        mut config: SolidityTestRunnerConfig<HardforkT>,
    ) -> MultiContractRunner<
        BlockT,
        ChainContextT,
        EvmBuilderT,
        HaltReasonT,
        HardforkT,
        NoOpContractDecoder<HaltReasonT>,
        TransactionErrorT,
        TransactionT,
    > {
        config.fuzz.seed = Some(seed);
        config.invariant = invariant_config.into();
        self.runner_with_config(config).await
    }

    /// Builds a tracing runner
    pub async fn tracing_runner(
        &self,
    ) -> MultiContractRunner<
        BlockT,
        ChainContextT,
        EvmBuilderT,
        HaltReasonT,
        HardforkT,
        NoOpContractDecoder<HaltReasonT>,
        TransactionErrorT,
        TransactionT,
    > {
        let mut config = self.config_with_mock_rpc();
        config.include_traces = IncludeTraces::All;
        self.build_runner(config).await
    }

    /// Builds a runner that runs against forked state
    pub async fn forked_runner(
        &self,
        rpc: &str,
    ) -> MultiContractRunner<
        BlockT,
        ChainContextT,
        EvmBuilderT,
        HaltReasonT,
        HardforkT,
        NoOpContractDecoder<HaltReasonT>,
        TransactionErrorT,
        TransactionT,
    > {
        let mut config = self.config_with_mock_rpc();

        config.evm_opts.fork_url = Some(rpc.to_string());

        self.build_runner(config).await
    }

    async fn build_runner(
        &self,
        config: SolidityTestRunnerConfig<HardforkT>,
    ) -> MultiContractRunner<
        BlockT,
        ChainContextT,
        EvmBuilderT,
        HaltReasonT,
        HardforkT,
        NoOpContractDecoder<HaltReasonT>,
        TransactionErrorT,
        TransactionT,
    > {
        MultiContractRunner::<
            BlockT,
            ChainContextT,
            EvmBuilderT,
            HaltReasonT,
            HardforkT,
            NoOpContractDecoder<HaltReasonT>,
            TransactionErrorT,
            TransactionT,
        >::new(
            config,
            self.test_contracts.clone(),
            self.known_contracts.clone(),
            self.libs_to_deploy.clone(),
            NoOpContractDecoder::default(),
            self.revert_decoder.clone(),
        )
        .await
        .expect("Config should be ok")
    }
}

fn get_compiled(project: &Project) -> ProjectCompileOutput {
    let lock_file_path = project.sources_path().join(".lock");
    // Compile only once per test run.
    // We need to use a file lock because `cargo-nextest` runs tests in different
    // processes. This is similar to [`edr_test_utils::util::initialize`],
    // see its comments for more details.
    let mut lock = new_fd_lock(&lock_file_path);
    let read = lock.read().unwrap();
    let out;
    if project.cache_path().exists() && std::fs::read(&lock_file_path).unwrap() == b"1" {
        out = project.compile();
        drop(read);
    } else {
        drop(read);
        let mut write = lock.write().unwrap();
        write.write_all(b"1").unwrap();
        out = project.compile();
        drop(write);
    }

    let out = out.unwrap();
    assert!(!out.has_compiler_errors(), "Compiled with errors:\n{out}");
    out
}

/// Default data for the tests group.
pub static TEST_DATA_DEFAULT: Lazy<L1ForgeTestData> = Lazy::new(|| {
    ForgeTestData::new(ForgeTestProfile::Default, l1::SpecId::CANCUN).expect("linking ok")
});

/// Data for tests requiring Cancun support on Solc and EVM level.
pub static TEST_DATA_CANCUN: Lazy<L1ForgeTestData> = Lazy::new(|| {
    ForgeTestData::new(ForgeTestProfile::Cancun, l1::SpecId::CANCUN).expect("linking ok")
});

/// Data for tests requiring Cancun support on Solc and EVM level.
pub static TEST_DATA_MULTI_VERSION: Lazy<L1ForgeTestData> = Lazy::new(|| {
    ForgeTestData::new(ForgeTestProfile::MultiVersion, l1::SpecId::CANCUN).expect("linking ok")
});

fn mock_rpc_endpoints() -> RpcEndpoints {
    RpcEndpoints::new([(
        "rpcAliasFake",
        RpcEndpoint::Url("https://example.com".to_string()),
    )])
}

fn remote_rpc_endpoints() -> RpcEndpoints {
    RpcEndpoints::new([
        (
            "rpcAliasMainnet",
            RpcEndpoint::Url(get_alchemy_url_for_network(NetworkType::Ethereum)),
        ),
        (
            "rpcAliasSepolia",
            RpcEndpoint::Url(get_alchemy_url_for_network(NetworkType::Sepolia)),
        ),
        (
            "rpcEnvAlias",
            RpcEndpoint::Env("${RPC_ENV_ALIAS}".to_string()),
        ),
        (
            "rpcAliasOptimism",
            RpcEndpoint::Url(get_alchemy_url_for_network(NetworkType::Optimism)),
        ),
        (
            "rpcAliasPolygon",
            RpcEndpoint::Url(get_alchemy_url_for_network(NetworkType::Polygon)),
        ),
        (
            "rpcAliasArbitrum",
            RpcEndpoint::Url(get_alchemy_url_for_network(NetworkType::Arbitrum)),
        ),
    ])
}
