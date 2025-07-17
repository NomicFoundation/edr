use std::path::PathBuf;

use edr_eth::{Address, U256};
use edr_solidity_tests::{
    backend::Predeploy,
    evm_context::HardforkTr,
    fuzz::{invariant::InvariantConfig, FuzzConfig},
    inspectors::cheatcodes::CheatsConfigOptions,
    IncludeTraces, SolidityTestRunnerConfig, SyncOnCollectedCoverageCallback, TestFilterConfig,
};
use napi::{bindgen_prelude::Uint8Array, Either};

/// Hardhat V3 build info where the compiler output is not part of the build
/// info file.
pub struct BuildInfoAndOutput {
    /// The build info input file
    pub build_info: Uint8Array,
    /// The build info output file
    pub output: Uint8Array,
}

impl<'a> From<&'a BuildInfoAndOutput>
    for edr_solidity::artifacts::BuildInfoBufferSeparateOutput<'a>
{
    fn from(value: &'a BuildInfoAndOutput) -> Self {
        Self {
            build_info: value.build_info.as_ref(),
            output: value.output.as_ref(),
        }
    }
}

/// Tracing config for Solidity stack trace generation.
pub struct TracingConfigWithBuffers {
    /// Build information to use for decoding contracts. Either a Hardhat v2
    /// build info file that contains both input and output or a Hardhat v3
    /// build info file that doesn't contain output and a separate output file.
    pub build_infos: Option<Either<Vec<Uint8Array>, Vec<BuildInfoAndOutput>>>,
    /// Whether to ignore contracts whose name starts with "Ignored".
    pub ignore_contracts: Option<bool>,
}

impl std::fmt::Debug for TracingConfigWithBuffers {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let build_infos = self.build_infos.as_ref().map_or_else(
            || "None".to_string(),
            |bi| match bi {
                Either::A(arrays) => format!("Uint8Array[{}]", arrays.len()),
                Either::B(build_infos) => format!("BuildInfoAndOutput[{}]", build_infos.len()),
            },
        );
        f.debug_struct("TracingConfigWithBuffers")
            .field("build_infos", &build_infos)
            .field("ignore_contracts", &self.ignore_contracts)
            .finish()
    }
}

impl<'a> From<&'a TracingConfigWithBuffers>
    for edr_solidity::artifacts::BuildInfoConfigWithBuffers<'a>
{
    fn from(value: &'a TracingConfigWithBuffers) -> Self {
        use edr_solidity::artifacts::{BuildInfoBufferSeparateOutput, BuildInfoBuffers};

        let build_infos = value.build_infos.as_ref().map(|infos| match infos {
            Either::A(with_output) => BuildInfoBuffers::WithOutput(
                with_output
                    .iter()
                    .map(std::convert::AsRef::as_ref)
                    .collect(),
            ),
            Either::B(separate_output) => BuildInfoBuffers::SeparateInputOutput(
                separate_output
                    .iter()
                    .map(BuildInfoBufferSeparateOutput::from)
                    .collect(),
            ),
        });

        Self {
            build_infos,
            ignore_contracts: value.ignore_contracts,
        }
    }
}

/// Solidity test runner configuration arguments exposed through the ffi.
/// Docs based on <https://book.getfoundry.sh/reference/config/testing>
pub struct TestRunnerConfig {
    /// The absolute path to the project root directory.
    /// Relative paths in cheat codes are resolved against this path.
    pub project_root: PathBuf,
    /// Whether to support the `testFail` prefix.
    pub test_fail: bool,
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
    pub sender: Option<Address>,
    /// The value of `tx.origin` in tests as hex string.
    /// Defaults to `0x1804c8AB1F12E6bbf3894d4083f33e07309d1f38`.
    pub tx_origin: Option<Address>,
    /// The initial balance of the sender in tests.
    /// Defaults to `0xffffffffffffffffffffffff`.
    pub initial_balance: Option<U256>,
    /// The value of `block.number` in tests.
    /// Defaults to `1`.
    pub block_number: Option<u64>,
    /// The value of the `chainid` opcode in tests.
    /// Defaults to `31337`.
    pub chain_id: Option<u64>,
    /// The gas limit for each test case.
    /// Defaults to `9_223_372_036_854_775_807` (`i64::MAX`).
    pub gas_limit: Option<u64>,
    /// The price of gas (in wei) in tests.
    /// Defaults to `0`.
    pub gas_price: Option<u64>,
    /// The base fee per gas (in wei) in tests.
    /// Defaults to `0`.
    pub block_base_fee_per_gas: Option<u64>,
    /// The value of `block.coinbase` in tests.
    /// Defaults to `0x0000000000000000000000000000000000000000`.
    pub block_coinbase: Option<Address>,
    /// The value of `block.timestamp` in tests.
    /// Defaults to 1.
    pub block_timestamp: Option<u64>,
    /// The value of `block.difficulty` in tests.
    /// Defaults to 0.
    pub block_difficulty: Option<u64>,
    /// The `block.gaslimit` value during EVM execution.
    /// Defaults to none.
    pub block_gas_limit: Option<u64>,
    /// Whether to disable the block gas limit.
    /// Defaults to false.
    pub disable_block_gas_limit: Option<bool>,
    /// The memory limit of the EVM in bytes.
    /// Defaults to `33_554_432` (2^25 = 32MiB).
    pub memory_limit: Option<u64>,
    /// The predeploys applied in local mode. Defaults to no predeploys.
    /// These should match the predeploys of the network in fork mode, so they
    /// aren't set in fork mode.
    pub local_predeploys: Option<Vec<Predeploy>>,
    /// If set, all tests are run in fork mode using this url or remote name.
    /// Defaults to none.
    pub fork_url: Option<String>,
    /// Pins the block number for the global state fork.
    pub fork_block_number: Option<u64>,
    /// Cheatcode configuration.
    pub cheatcode: CheatsConfigOptions,
    /// Fuzz testing configuration.
    pub fuzz: FuzzConfig,
    /// Invariant testing configuration.
    /// If an invariant config setting is not set, but a corresponding fuzz
    /// config value is set, then the fuzz config value will be used.
    pub invariant: InvariantConfig,
    /// Whether to enable trace mode and which traces to include in test
    /// results.
    pub include_traces: IncludeTraces,
    /// The configuration for the Solidity test runner's observability
    pub on_collected_coverage_fn: Option<Box<dyn SyncOnCollectedCoverageCallback>>,
    /// A regex pattern to filter tests. If provided, only test methods that
    /// match the pattern will be executed and reported as a test result.
    pub test_pattern: TestFilterConfig,
}

impl<HardforkT: HardforkTr> TryFrom<TestRunnerConfig> for SolidityTestRunnerConfig<HardforkT> {
    type Error = napi::Error;

    fn try_from(value: TestRunnerConfig) -> Result<Self, Self::Error> {
        let TestRunnerConfig {
            project_root,
            test_fail,
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
            local_predeploys,
            fork_url,
            fork_block_number,
            cheatcode: cheats_config_options,
            fuzz,
            invariant,
            include_traces,
            on_collected_coverage_fn,
            test_pattern: _,
        } = value;

        let mut evm_opts = SolidityTestRunnerConfig::default_evm_opts();

        if let Some(gas_limit) = gas_limit {
            evm_opts.env.gas_limit = gas_limit;
        }

        evm_opts.env.chain_id = chain_id;

        evm_opts.env.gas_price = gas_price;

        if let Some(block_base_fee_per_gas) = block_base_fee_per_gas {
            evm_opts.env.block_base_fee_per_gas = block_base_fee_per_gas;
        }

        if let Some(tx_origin) = tx_origin {
            evm_opts.env.tx_origin = tx_origin;
        }

        if let Some(block_number) = block_number {
            evm_opts.env.block_number = block_number;
        }

        if let Some(block_difficulty) = block_difficulty {
            evm_opts.env.block_difficulty = block_difficulty;
        }

        evm_opts.env.block_gas_limit = block_gas_limit;

        if let Some(block_timestamp) = block_timestamp {
            evm_opts.env.block_timestamp = block_timestamp;
        }

        if let Some(block_coinbase) = block_coinbase {
            evm_opts.env.block_coinbase = block_coinbase;
        }

        evm_opts.fork_url = fork_url;

        evm_opts.fork_block_number = fork_block_number;

        if let Some(isolate) = isolate {
            evm_opts.isolate = isolate;
        }

        if let Some(ffi) = ffi {
            evm_opts.ffi = ffi;
        }

        if let Some(sender) = sender {
            evm_opts.sender = sender;
        }

        if let Some(initial_balance) = initial_balance {
            evm_opts.initial_balance = initial_balance;
        }

        if let Some(memory_limit) = memory_limit {
            evm_opts.memory_limit = memory_limit;
        }

        if let Some(disable_block_gas_limit) = disable_block_gas_limit {
            evm_opts.disable_block_gas_limit = disable_block_gas_limit;
        }

        let local_predeploys = local_predeploys.unwrap_or_default();

        Ok(SolidityTestRunnerConfig {
            project_root,
            include_traces,
            test_fail,
            // TODO
            coverage: false,
            cheats_config_options,
            evm_opts,
            local_predeploys,
            fuzz,
            invariant,
            on_collected_coverage_fn,
            // Solidity fuzz fixtures are not supported by the JS backend
            solidity_fuzz_fixtures: false,
        })
    }
}
