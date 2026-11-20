use std::{
    collections::{BTreeMap, HashMap},
    path::{Path, PathBuf},
};

pub use edr_coverage::reporter::SyncOnCollectedCoverageCallback;
use edr_eip8037::TransactionGasBounds;
use edr_primitives::{Address, B256, U256};
use edr_solidity::config::IncludeTraces;
use edr_solidity_parser_slang::compilation::file_id_of;
pub use edr_solidity_parser_slang::exclusions::InvalidExclusionGlob;
use foundry_evm::{
    backend::Predeploy,
    evm_context::{BlockEnvTr, HardforkTr, TransactionEnvTr},
    fuzz::{invariant::InvariantConfig, FuzzConfig},
    inspectors::cheatcodes::CheatsConfigOptions,
};

use crate::{
    fork::CreateFork,
    inline_config::{ImportResolver, InlineConfigProfiles},
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
    /// An EIP-712 source exclusion glob could not be compiled.
    #[error(transparent)]
    InvalidEip712ExcludeGlob(#[from] InvalidExclusionGlob),
    /// Several `testSourcePaths` entries name one file.
    #[error(
        "`testSourcePaths` entries {} name the same file: {file_id}; one source name per file",
        quoted_sources(.sources)
    )]
    TestSourcePathShared {
        /// The shared file, spelled the way the parser identifies it.
        file_id: String,
        /// The source names pointing at it, sorted.
        sources: Vec<PathBuf>,
    },
}

/// Renders source names as a backtick-quoted list: ``a`, `b` and `c``.
fn quoted_sources(sources: &[PathBuf]) -> String {
    let quoted: Vec<String> = sources
        .iter()
        .map(|source| format!("`{}`", source.display()))
        .collect();

    match quoted.split_last() {
        None => String::new(),
        Some((last, [])) => last.clone(),
        Some((last, leading)) => format!("{} and {last}", leading.join(", ")),
    }
}

/// The test sources to collect from: which file each solc source name is read
/// from, holding one source name per file.
///
/// [`TestSourcePaths::try_from`] is the only way to build a non-empty one, so
/// a file two source names point at is rejected there rather than reaching
/// collection, which parses a file once and serves it under one name.
#[derive(Clone, Debug, Default)]
pub struct TestSourcePaths {
    /// Keyed by file id, so a second source name for a file is
    /// unrepresentable rather than caught downstream.
    entries: HashMap<String, TestSourceEntry>,
    /// The file id of each source name, the inverse of [`Self::entries`],
    /// because a lookup has a source name in hand.
    file_ids: HashMap<PathBuf, String>,
}

/// What one [`TestSourcePaths`] entry holds beyond the file id it is keyed by.
#[derive(Clone, Debug)]
struct TestSourceEntry {
    /// The solc source name the file is collected under.
    source: PathBuf,
    /// The path the file is read from.
    path: PathBuf,
}

/// The file one test source name is read from.
#[derive(Clone, Copy, Debug)]
pub struct TestSourceFile<'a> {
    /// The identity the parser gives the file. Collection keys one root by it.
    pub file_id: &'a str,
    /// The path the file is read from.
    pub path: &'a Path,
}

impl TestSourcePaths {
    /// Whether collection is disabled, which no entries does.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// The file `source` is read from, or `None` when it has no entry.
    pub fn get(&self, source: &Path) -> Option<TestSourceFile<'_>> {
        let file_id = self.file_ids.get(source)?;
        let entry = self
            .entries
            .get(file_id)
            .expect("every source name's file id is an entry");

        Some(TestSourceFile {
            file_id,
            path: &entry.path,
        })
    }

    /// Every source name with an entry.
    pub fn source_names(&self) -> impl Iterator<Item = &Path> {
        self.entries.values().map(|entry| entry.source.as_path())
    }

    /// Rebuilds the map these entries were built from, for a caller amending
    /// it. The amended map is validated again on its way back in.
    pub fn to_map(&self) -> HashMap<PathBuf, PathBuf> {
        self.entries
            .values()
            .map(|entry| (entry.source.clone(), entry.path.clone()))
            .collect()
    }
}

impl TryFrom<HashMap<PathBuf, PathBuf>> for TestSourcePaths {
    type Error = SolidityTestRunnerConfigError;

    /// Groups the entries by the file id of their path, the identity the
    /// parser gives a file, so `a/../b.sol` and `b.sol` count as one file.
    ///
    /// Reports the first clash in file-id order, like the sibling
    /// configuration problems, which also reject at the first one.
    fn try_from(test_source_paths: HashMap<PathBuf, PathBuf>) -> Result<Self, Self::Error> {
        let mut entries = HashMap::with_capacity(test_source_paths.len());
        let mut sources_by_file_id: BTreeMap<String, Vec<PathBuf>> = BTreeMap::new();
        for (source, path) in test_source_paths {
            let file_id = file_id_of(&path);
            sources_by_file_id
                .entry(file_id.clone())
                .or_default()
                .push(source.clone());
            entries.insert(file_id, TestSourceEntry { source, path });
        }

        if let Some((file_id, mut sources)) = sources_by_file_id
            .into_iter()
            .find(|(_, sources)| sources.len() > 1)
        {
            sources.sort();

            return Err(SolidityTestRunnerConfigError::TestSourcePathShared { file_id, sources });
        }

        let file_ids = entries
            .iter()
            .map(|(file_id, entry)| (entry.source.clone(), file_id.clone()))
            .collect();

        Ok(Self { entries, file_ids })
    }
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
    /// Maps each test source's solc source name to its absolute path on disk,
    /// used to read and parse the source for inline configuration
    /// (`forge-config:`/`hardhat-config:` NatSpec directives) and for the
    /// EIP-712 struct definitions served to the `eip712HashType` and
    /// `eip712HashStruct` cheatcodes.
    ///
    /// No entries disables collection: no source is read or parsed. A
    /// non-empty map must name the source of every test suite a run selects,
    /// and every source backing a selected suite is parsed. An entry no
    /// selected suite uses is never opened. One source name per file, which
    /// [`TestSourcePaths`] enforces when it is built.
    ///
    /// Each problem is accumulated and reported together when the run starts,
    /// rather than silently leaving a suite without inline configuration or
    /// EIP-712 types. A suite with no entry, an unreadable file, a solc
    /// version older than 0.8 and a source that does not parse all count.
    ///
    /// Only the sources of the suites a run selects are read and parsed, so a
    /// filtered run does not pay for the rest of the project.
    pub test_source_paths: TestSourcePaths,
    /// Resolves the imports of test sources while parsing them (see
    /// [`Self::test_source_paths`]).
    pub import_resolver: ImportResolver,
    /// Globs matched against each parsed source's path relative to
    /// [`Self::project_root`], spelled with forward slashes. A file outside
    /// the root is matched by its absolute path.
    ///
    /// A struct declared in a matching file is not collected, including when
    /// the matching file is the test's own source. Excluding is how a project
    /// settles a same-name clash between two reachable structs. Nothing is
    /// excluded by default, and there is no opposite setting, because every
    /// reachable struct is collected.
    ///
    /// Only EIP-712 collection is affected. An excluded source still
    /// contributes its inline configuration and is still parsed, so a syntax
    /// error in it still rejects the run.
    pub eip712_exclude_globs: Vec<String>,
    /// The profile the run was started with and every profile the project
    /// declares, used to scope inline-config directives. Defaults to the single
    /// `default` profile.
    pub inline_config_profiles: InlineConfigProfiles,
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

/// The gas limit Solidity tests run with when the hardfork imposes no bound on
/// `tx.gas`. Tests intentionally use a very large limit so they never run out
/// of gas.
pub const MAX_TEST_TRANSACTION_GAS_LIMIT: u64 = i64::MAX as u64;

impl<HardforkT: HardforkTr> SolidityTestRunnerConfig<HardforkT> {
    /// The default evm options for the Solidity test runner.
    pub fn default_evm_opts() -> EvmOpts<HardforkT> {
        let spec = HardforkT::default();
        let transaction_gas_bounds = TransactionGasBounds::for_hardfork(spec);
        // Solidity tests want as much gas as possible, so the default gas limit
        // is the most a transaction may carry on the hardfork, or the maximum.
        let gas_limit = transaction_gas_bounds
            .total_transaction_gas
            .unwrap_or(MAX_TEST_TRANSACTION_GAS_LIMIT);

        EvmOpts {
            env: EvmEnv {
                gas_limit,
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
            spec,
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
            transaction_execution_gas_bound: transaction_gas_bounds.execution_gas,
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

/// Config override for a single test. Also used as a contract-level default
/// (see [`Self::fill_unset_from`]).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TestFunctionConfigOverride {
    /// Allow expecting reverts with `expectRevert` at the same callstack depth
    /// as the test.
    pub allow_internal_expect_revert: Option<bool>,
    /// Whether to enable isolation of calls for the test. In isolation mode all
    /// top-level calls are executed as a separate transaction in a separate
    /// EVM context, enabling more precise gas accounting and transaction
    /// state changes.
    /// Ignored when gas reporting is enabled, as isolation is required for
    /// accurate gas measurements.
    pub isolate: Option<bool>,
    /// The EVM version to use for this test, e.g. "Cancun". This will override
    /// the global EVM version.
    pub evm_version: Option<String>,
    /// Configuration override for fuzz testing
    pub fuzz: Option<FuzzConfigOverride>,
    /// Configuration override for invariant testing
    pub invariant: Option<InvariantConfigOverride>,
}

/// Sets `field` to `fallback`'s value if it is unset.
fn fill<T: Clone>(field: &mut Option<T>, fallback: &Option<T>) {
    if field.is_none() {
        field.clone_from(fallback);
    }
}

impl TestFunctionConfigOverride {
    /// Fills every unset value from `fallback`, key by key — including within
    /// the fuzz and invariant sections. Used to apply a contract-level inline
    /// configuration underneath a function-level one: values set on the
    /// function win, everything else falls back to the contract's.
    pub fn fill_unset_from(&mut self, fallback: &Self) {
        let Self {
            allow_internal_expect_revert,
            isolate,
            evm_version,
            fuzz,
            invariant,
        } = fallback;

        fill(
            &mut self.allow_internal_expect_revert,
            allow_internal_expect_revert,
        );
        fill(&mut self.isolate, isolate);
        fill(&mut self.evm_version, evm_version);

        if let Some(fallback_fuzz) = fuzz {
            match &mut self.fuzz {
                Some(fuzz) => fuzz.fill_unset_from(fallback_fuzz),
                None => self.fuzz = Some(fallback_fuzz.clone()),
            }
        }
        if let Some(fallback_invariant) = invariant {
            match &mut self.invariant {
                Some(invariant) => invariant.fill_unset_from(fallback_invariant),
                None => self.invariant = Some(fallback_invariant.clone()),
            }
        }
    }
}

impl FuzzConfigOverride {
    /// Fills every unset value from `fallback` (see
    /// [`TestFunctionConfigOverride::fill_unset_from`]).
    fn fill_unset_from(&mut self, fallback: &Self) {
        let Self {
            runs,
            max_test_rejects,
            show_logs,
            timeout,
        } = fallback;

        fill(&mut self.runs, runs);
        fill(&mut self.max_test_rejects, max_test_rejects);
        fill(&mut self.show_logs, show_logs);
        fill(&mut self.timeout, timeout);
    }
}

impl InvariantConfigOverride {
    /// Fills every unset value from `fallback` (see
    /// [`TestFunctionConfigOverride::fill_unset_from`]).
    fn fill_unset_from(&mut self, fallback: &Self) {
        let Self {
            runs,
            depth,
            fail_on_revert,
            call_override,
            timeout,
        } = fallback;

        fill(&mut self.runs, runs);
        fill(&mut self.depth, depth);
        fill(&mut self.fail_on_revert, fail_on_revert);
        fill(&mut self.call_override, call_override);
        fill(&mut self.timeout, timeout);
    }
}

/// Timeout configuration.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TimeoutConfig {
    /// Optional timeout (in seconds)
    pub time: Option<u32>,
}

/// Test function or test contract level fuzz config override.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct FuzzConfigOverride {
    /// The number of test cases that must execute for each property test
    pub runs: Option<u32>,
    /// The maximum number of test case rejections allowed, to be encountered
    /// during usage of `vm.assume` cheatcode. `0` disables the limit and
    /// requires a `timeout`.
    pub max_test_rejects: Option<u32>,
    /// Show `console.log` in fuzz test.
    pub show_logs: Option<bool>,
    /// Optional timeout (in seconds) for each property test
    pub timeout: Option<TimeoutConfig>,
}

/// Test function or test contract level invariant config override.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fill_unset_from_keeps_set_values_and_fills_unset_ones() {
        let mut config = TestFunctionConfigOverride {
            isolate: Some(false),
            fuzz: Some(FuzzConfigOverride {
                runs: Some(1),
                ..FuzzConfigOverride::default()
            }),
            ..TestFunctionConfigOverride::default()
        };
        let fallback = TestFunctionConfigOverride {
            isolate: Some(true),
            evm_version: Some("cancun".to_owned()),
            fuzz: Some(FuzzConfigOverride {
                runs: Some(2),
                max_test_rejects: Some(3),
                ..FuzzConfigOverride::default()
            }),
            invariant: Some(InvariantConfigOverride {
                depth: Some(4),
                ..InvariantConfigOverride::default()
            }),
            ..TestFunctionConfigOverride::default()
        };

        config.fill_unset_from(&fallback);

        // Values set on `config` win, even against a conflicting fallback.
        assert_eq!(config.isolate, Some(false));
        let fuzz = config.fuzz.as_ref().unwrap();
        assert_eq!(fuzz.runs, Some(1));
        // Unset values fall back, key by key — also within a section `config`
        // already declares.
        assert_eq!(fuzz.max_test_rejects, Some(3));
        assert_eq!(config.evm_version.as_deref(), Some("cancun"));
        assert_eq!(config.invariant.as_ref().unwrap().depth, Some(4));
    }

    #[test]
    fn fill_unset_from_empty_config_takes_fallback() {
        let mut config = TestFunctionConfigOverride::default();
        let fallback = TestFunctionConfigOverride {
            allow_internal_expect_revert: Some(true),
            invariant: Some(InvariantConfigOverride {
                runs: Some(5),
                timeout: Some(TimeoutConfig { time: Some(1) }),
                ..InvariantConfigOverride::default()
            }),
            ..TestFunctionConfigOverride::default()
        };

        config.fill_unset_from(&fallback);
        assert_eq!(config, fallback);
    }

    #[test]
    fn fill_unset_from_empty_fallback_changes_nothing() {
        let mut config = TestFunctionConfigOverride {
            isolate: Some(true),
            ..TestFunctionConfigOverride::default()
        };
        let before = config.clone();
        config.fill_unset_from(&TestFunctionConfigOverride::default());
        assert_eq!(config, before);
    }

    /// The entries of a map naming one file per source name.
    fn source_paths(entries: &[(&str, &str)]) -> HashMap<PathBuf, PathBuf> {
        entries
            .iter()
            .map(|(source, path)| (PathBuf::from(source), PathBuf::from(path)))
            .collect()
    }

    #[test]
    fn distinct_files_are_accepted() {
        let paths = TestSourcePaths::try_from(source_paths(&[
            ("test/A.t.sol", "/project/test/A.t.sol"),
            ("test/B.t.sol", "/project/test/B.t.sol"),
        ]))
        .expect("each source name has its own file");

        assert!(!paths.is_empty());
        let file = paths
            .get(Path::new("test/A.t.sol"))
            .expect("`test/A.t.sol` has an entry");
        assert_eq!(file.path, Path::new("/project/test/A.t.sol"));
        assert_eq!(file.file_id, file_id_of(Path::new("/project/test/A.t.sol")));
        assert!(paths.get(Path::new("test/Missing.t.sol")).is_none());

        let mut source_names: Vec<&Path> = paths.source_names().collect();
        source_names.sort_unstable();
        assert_eq!(
            source_names,
            vec![Path::new("test/A.t.sol"), Path::new("test/B.t.sol")]
        );
    }

    #[test]
    fn no_entries_disable_collection() {
        let paths = TestSourcePaths::try_from(HashMap::new()).expect("an empty map is valid");

        assert!(paths.is_empty());
        assert!(TestSourcePaths::default().is_empty());
    }

    #[test]
    fn two_source_names_for_one_file_are_rejected() {
        let shared = "/project/test/Shared.t.sol";

        let error = TestSourcePaths::try_from(source_paths(&[
            ("test/B.t.sol", shared),
            ("test/A.t.sol", shared),
        ]))
        .expect_err("one file backs two source names");

        assert!(
            matches!(
                &error,
                SolidityTestRunnerConfigError::TestSourcePathShared { file_id, sources }
                    if *file_id == file_id_of(Path::new(shared))
                        && *sources == vec![
                            PathBuf::from("test/A.t.sol"),
                            PathBuf::from("test/B.t.sol"),
                        ]
            ),
            "{error:?}"
        );
        assert_eq!(
            error.to_string(),
            format!(
                "`testSourcePaths` entries `test/A.t.sol` and `test/B.t.sol` name the same file: {}; one source name per file",
                file_id_of(Path::new(shared))
            )
        );
    }

    /// A third name joins the list with a comma, while two keep the bare
    /// `a` and `b` form.
    #[test]
    fn three_source_names_for_one_file_are_listed_with_a_comma() {
        let shared = "/project/test/Shared.t.sol";

        let error = TestSourcePaths::try_from(source_paths(&[
            ("test/C.t.sol", shared),
            ("test/A.t.sol", shared),
            ("test/B.t.sol", shared),
        ]))
        .expect_err("one file backs three source names");

        assert!(
            error
                .to_string()
                .starts_with("`testSourcePaths` entries `test/A.t.sol`, `test/B.t.sol` and `test/C.t.sol` name the same file"),
            "{error}"
        );
    }

    /// Paths are compared by file id, so two spellings of one file are a
    /// clash rather than two files.
    #[test]
    fn two_spellings_of_one_file_are_rejected() {
        let error = TestSourcePaths::try_from(source_paths(&[
            ("test/A.t.sol", "/project/test/A.t.sol"),
            ("test/B.t.sol", "/project/test/../test/A.t.sol"),
        ]))
        .expect_err("both spellings name one file");

        assert!(
            error
                .to_string()
                .contains("`test/A.t.sol` and `test/B.t.sol` name the same file"),
            "{error}"
        );
    }
}
