//! The Forge test runner.
use std::{borrow::Cow, cmp::min, collections::BTreeMap, path::Path, sync::Arc, time::Instant};

use alloy_dyn_abi::DynSolValue;
use alloy_json_abi::Function;
use alloy_primitives::{map::AddressHashMap, Address, Bytes, Log, U256};
use edr_solidity::{
    contract_decoder::{NestedTraceDecoder, SyncNestedTraceDecoder},
    solidity_stack_trace::StackTraceEntry,
};
use eyre::Result;
use foundry_evm::{
    abi::TestFunctionExt,
    constants::{CALLER, LIBRARY_DEPLOYER},
    contracts::{ContractsByAddress, ContractsByArtifact},
    coverage::HitMaps,
    decode::{decode_console_logs, RevertDecoder},
    executors::{
        fuzz::FuzzedExecutor,
        invariant::{
            check_sequence, replay_error, replay_run, InvariantConfig, InvariantExecutor,
            InvariantFuzzError, InvariantFuzzTestResult, ReplayErrorArgs, ReplayResult,
            ReplayRunArgs,
        },
        stack_trace::{get_stack_trace, StackTraceError, StackTraceResult},
        CallResult, EvmError, ExecutionErr, Executor, ExecutorBuilder, RawCallResult,
    },
    fuzz::{
        fixture_name,
        invariant::{CallDetails, InvariantContract},
        CounterExample, FuzzFixtures,
    },
    traces::{load_contracts, TraceKind},
};
use proptest::test_runner::{TestError, TestRunner};
use rayon::prelude::*;
use serde::{Deserialize, Serialize};

use crate::{
    fuzz::{invariant::BasicTxDetails, BaseCounterExample, FuzzConfig},
    multi_runner::TestContract,
    result::{SuiteResult, TestKind, TestResult, TestSetup, TestStatus},
    traces::Traces,
    TestFilter, TestOptions,
};

/// A type that executes all tests of a contract
#[derive(Clone, Debug)]
pub struct ContractRunner<'a, NestedTraceDecoderT> {
    pub name: &'a str,
    /// The data of the contract being ran.
    pub contract: &'a TestContract,
    /// Revert decoder. Contains all known errors.
    pub revert_decoder: &'a RevertDecoder,
    /// Known contracts by artifact id
    pub known_contracts: &'a ContractsByArtifact,
    /// Libraries to deploy.
    pub libs_to_deploy: &'a [Bytes],
    /// Provides contract metadata from calldata and traces.
    pub contract_decoder: Arc<NestedTraceDecoderT>,
    /// The initial balance of the test contract
    pub initial_balance: U256,
    /// The address which will be used as the `from` field in all EVM calls
    pub sender: Address,
    /// Whether to support the `testFail` prefix
    pub test_fail: bool,
    /// Whether to enable solidity fuzz fixtures support
    pub solidity_fuzz_fixtures: bool,

    /// The config values required to build the executor.
    executor_builder: ExecutorBuilder,
}

/// Options for [`ContractRunner`].
#[derive(Clone, Debug)]
pub struct ContractRunnerOptions {
    /// The initial balance of the test contract
    pub initial_balance: U256,
    /// The address which will be used as the `from` field in all EVM calls
    pub sender: Address,
    /// Whether to support the `testFail` prefix
    pub test_fail: bool,
    /// whether to enable solidity fuzz fixtures support
    pub solidity_fuzz_fixtures: bool,
}

/// Contract artifact related argumetns to the contract runner.
pub struct ContractRunnerArtifacts<'a, NestedTracerDecoderT: SyncNestedTraceDecoder> {
    pub revert_decoder: &'a RevertDecoder,
    pub known_contracts: &'a ContractsByArtifact,
    pub libs_to_deploy: &'a [Bytes],
    pub contract_decoder: Arc<NestedTracerDecoderT>,
}

impl<'a, NestedTracerDecoderT: SyncNestedTraceDecoder> ContractRunner<'a, NestedTracerDecoderT> {
    pub fn new(
        name: &'a str,
        executor_builder: ExecutorBuilder,
        contract: &'a TestContract,
        artifacts: ContractRunnerArtifacts<'a, NestedTracerDecoderT>,
        options: ContractRunnerOptions,
    ) -> Self {
        let ContractRunnerArtifacts {
            revert_decoder,
            known_contracts,
            libs_to_deploy,
            contract_decoder,
        } = artifacts;
        let ContractRunnerOptions {
            initial_balance,
            sender,
            test_fail,
            solidity_fuzz_fixtures,
        } = options;

        Self {
            name,
            contract,
            revert_decoder,
            known_contracts,
            libs_to_deploy,
            contract_decoder,
            initial_balance,
            sender,
            test_fail,
            solidity_fuzz_fixtures,
            executor_builder,
        }
    }
}

impl<NestedTraceDecoderT: SyncNestedTraceDecoder> ContractRunner<'_, NestedTraceDecoderT> {
    /// Runs all tests for a contract whose names match the provided regular
    /// expression
    pub fn run_tests(
        self,
        filter: &dyn TestFilter,
        test_options: &TestOptions,
        handle: &tokio::runtime::Handle,
    ) -> SuiteResult {
        info!("starting tests");
        let start = Instant::now();
        let mut warnings = Vec::new();

        let mut executor = self.executor_builder.clone().build();

        let setup_fns: Vec<_> = self
            .contract
            .abi
            .functions()
            .filter(|func| func.name.is_setup())
            .collect();

        let needs_setup = setup_fns.len() == 1 && setup_fns[0].name == "setUp";

        // There is a single miss-cased `setUp` function, so we add a warning
        for &setup_fn in setup_fns.iter() {
            if setup_fn.name != "setUp" {
                warnings.push(format!(
                    "Found invalid setup function \"{}\" did you mean \"setUp()\"?",
                    setup_fn.signature()
                ));
            }
        }

        // There are multiple setUp function, so we return a single test result for
        // `setUp`
        if setup_fns.len() > 1 {
            return SuiteResult::new(
                start.elapsed(),
                [(
                    "setUp()".to_string(),
                    TestResult::fail("multiple setUp functions".to_string()),
                )]
                .into(),
                warnings,
            );
        }

        // Check if `afterInvariant` function with valid signature declared.
        let after_invariant_fns: Vec<_> = self
            .contract
            .abi
            .functions()
            .filter(|func| func.name.is_after_invariant())
            .collect();
        if after_invariant_fns.len() > 1 {
            // Return a single test result failure if multiple functions declared.
            return SuiteResult::new(
                start.elapsed(),
                [(
                    "afterInvariant()".to_string(),
                    TestResult::fail("multiple afterInvariant functions".to_string()),
                )]
                .into(),
                warnings,
            );
        }
        let call_after_invariant = after_invariant_fns.first().is_some_and(|after_invariant_fn| {
            let match_sig = after_invariant_fn.name == "afterInvariant";
            if !match_sig {
                warnings.push(format!(
                    "Found invalid afterInvariant function \"{}\" did you mean \"afterInvariant()\"?",
                    after_invariant_fn.signature()
                ));
            }
            match_sig
        });

        let has_invariants = self
            .contract
            .abi
            .functions()
            .any(foundry_evm::abi::TestFunctionExt::is_invariant_test);

        // Invariant testing requires tracing to figure out what contracts were created.
        // This would be only strictly needed for invariant tests if there are contracts
        // created in the `setUp()` method. We do it even if there is no `setUp` method,
        // because stack trace generation needs setup traces as well to match addresses
        // to contract code, and it simplifies re-execution for invariant tests
        // if we don't need to redo the setup.
        let setup_tracing = executor.inspector.tracer.is_none() && has_invariants;
        if setup_tracing {
            executor.set_tracing(true);
        }
        let setup = self.setup(&mut executor, needs_setup);
        if setup_tracing {
            executor.set_tracing(false);
        }

        if setup.reason.is_some() {
            // We want to report execution time without stack trace generation as people use
            // these numbers to reason about the performance of their code.
            let elapsed = start.elapsed();

            // Re-execute for stack traces
            let stack_trace_result: StackTraceResult =
                if let Some(indeterminism_reasons) = executor.indeterminism_reasons() {
                    indeterminism_reasons.into()
                } else {
                    let mut executor = self.executor_builder.clone().build();
                    executor.inspector.enable_for_stack_traces();
                    let setup_for_stack_traces = self.setup(&mut executor, needs_setup);

                    get_stack_trace(&*self.contract_decoder, &setup_for_stack_traces.traces)
                        .transpose()
                        .expect("traces are not empty")
                        .into()
                };

            // The setup failed, so we return a single test result for `setUp`
            return SuiteResult::new(
                elapsed,
                [(
                    "setUp()".to_string(),
                    TestResult {
                        status: TestStatus::Failure,
                        reason: setup.reason,
                        counterexample: None,
                        decoded_logs: decode_console_logs(&setup.logs),
                        logs: setup.logs,
                        kind: TestKind::Standard(0),
                        traces: setup.traces,
                        gas_report_traces: vec![],
                        coverage: setup.coverage,
                        labeled_addresses: setup.labeled_addresses,
                        duration: elapsed,
                        stack_trace_result: Some(stack_trace_result),
                    },
                )]
                .into(),
                warnings,
            );
        }

        // Filter out functions sequentially since it's very fast and there is no need
        // to do it in parallel.
        let find_timer = Instant::now();
        let functions = self
            .contract
            .abi
            .functions()
            .filter(|func| is_matching_test(func, filter))
            .collect::<Vec<_>>();
        let find_time = find_timer.elapsed();
        debug!(
            "Found {} test functions out of {} in {:?}",
            functions.len(),
            self.contract.abi.functions().count(),
            find_time,
        );

        let identified_contracts = has_invariants.then(|| {
            load_contracts(
                setup.traces.iter().map(|(_, t)| &t.arena),
                self.known_contracts,
            )
        });
        let test_results = functions
            .par_iter()
            .map(|&func| {
                let _guard = handle.enter();

                let sig = func.signature();

                let setup = setup.clone();
                let should_fail = self.test_fail && func.is_any_test_fail();
                let res = if func.is_invariant_test() {
                    let runner = test_options.invariant_runner();
                    let invariant_config = test_options.invariant_config();
                    self.run_invariant_test(RunInvariantTestsArgs {
                        executor: executor.clone(),
                        test_bytecode: &self.contract.bytecode,
                        runner,
                        setup,
                        invariant_config: invariant_config.clone(),
                        func,
                        call_after_invariant,
                        known_contracts: self.known_contracts,
                        identified_contracts: identified_contracts
                            .as_ref()
                            .expect("invariant tests have identified contracts"),
                    })
                } else if func.is_fuzz_test() {
                    let runner = test_options.fuzz_runner();
                    let fuzz_config = test_options.fuzz_config();
                    self.run_fuzz_test(
                        executor.clone(),
                        func,
                        should_fail,
                        runner,
                        setup,
                        fuzz_config.clone(),
                    )
                } else {
                    debug_assert!(func.is_unit_test());
                    self.run_test(executor.clone(), func, should_fail, setup)
                };

                (sig, res)
            })
            .collect::<BTreeMap<_, _>>();

        let duration = start.elapsed();
        let suite_result = SuiteResult::new(duration, test_results, warnings);
        info!(
            duration=?suite_result.duration,
            "done. {}/{} successful",
            suite_result.passed(),
            suite_result.test_results.len()
        );
        suite_result
    }

    /// Deploys the test contract inside the runner from the sending account,
    /// and optionally runs the `setUp` function on the test contract.
    fn setup(&self, executor: &mut Executor, needs_setup: bool) -> TestSetup {
        self._setup(executor, needs_setup)
            .unwrap_or_else(|err| TestSetup::failed(err.to_string()))
    }

    fn _setup(&self, executor: &mut Executor, needs_setup: bool) -> Result<TestSetup> {
        trace!(?needs_setup, "Setting test contract");

        // We max out their balance so that they can deploy and make calls.
        executor.set_balance(self.sender, U256::MAX)?;
        executor.set_balance(CALLER, U256::MAX)?;

        // We set the nonce of the deployer accounts to 1 to get the same addresses as
        // DappTools
        executor.set_nonce(self.sender, 1)?;

        // Deploy libraries
        // +1 for contract deployment
        let capacity = self.libs_to_deploy.len() + 1 + usize::from(needs_setup);
        let mut logs = Vec::with_capacity(capacity);
        let mut traces = Vec::with_capacity(capacity);
        let mut deployed_libs = Vec::with_capacity(capacity);
        for code in self.libs_to_deploy.iter() {
            match executor.deploy(
                LIBRARY_DEPLOYER,
                code.clone(),
                U256::ZERO,
                Some(self.revert_decoder),
            ) {
                Ok(d) => {
                    logs.extend(d.raw.logs);
                    traces.extend(d.raw.traces.map(|traces| (TraceKind::Deployment, traces)));
                    deployed_libs.push(d.address);
                }
                Err(e) => {
                    return Ok(TestSetup::from_evm_error_with(
                        e,
                        logs,
                        traces,
                        AddressHashMap::default(),
                        deployed_libs,
                        needs_setup,
                    ))
                }
            }
        }

        let address = self.sender.create(executor.get_nonce(self.sender)?);

        // Set the contracts initial balance before deployment, so it is available
        // during construction
        executor.set_balance(address, self.initial_balance)?;

        // Deploy the test contract
        match executor.deploy(
            self.sender,
            self.contract.bytecode.clone(),
            U256::ZERO,
            Some(self.revert_decoder),
        ) {
            Ok(d) => {
                logs.extend(d.raw.logs);
                traces.extend(d.raw.traces.map(|traces| (TraceKind::Deployment, traces)));
                d.address
            }
            Err(e) => {
                return Ok(TestSetup::from_evm_error_with(
                    e,
                    logs,
                    traces,
                    AddressHashMap::default(),
                    deployed_libs,
                    needs_setup,
                ))
            }
        };

        // Reset `self.sender`s and `CALLER`s balance to the initial balance we want
        executor.set_balance(self.sender, self.initial_balance)?;
        executor.set_balance(CALLER, self.initial_balance)?;
        executor.set_balance(LIBRARY_DEPLOYER, self.initial_balance)?;

        executor.deploy_create2_deployer()?;

        // Optionally call the `setUp` function
        let setup = if needs_setup {
            trace!("setting up");
            let res = executor.setup(None, address, Some(self.revert_decoder));
            let (setup_logs, setup_traces, labeled_addresses, reason, coverage) = match res {
                Ok(RawCallResult {
                    traces,
                    labels,
                    logs,
                    coverage,
                    ..
                }) => {
                    trace!(contract=%address, "successfully setUp test");
                    (logs, traces, labels, None, coverage)
                }
                Err(EvmError::Execution(err)) => {
                    let ExecutionErr {
                        raw:
                            RawCallResult {
                                traces,
                                labels,
                                logs,
                                coverage,
                                ..
                            },
                        reason,
                    } = *err;
                    (logs, traces, labels, Some(reason), coverage)
                }
                Err(err) => (
                    Vec::new(),
                    None,
                    AddressHashMap::default(),
                    Some(err.to_string()),
                    None,
                ),
            };
            traces.extend(setup_traces.map(|traces| (TraceKind::Setup, traces)));
            logs.extend(setup_logs);

            TestSetup {
                address,
                logs,
                traces,
                labeled_addresses,
                reason,
                coverage,
                deployed_libs,
                fuzz_fixtures: self.fuzz_fixtures(executor, address),
                has_setup_method: needs_setup,
            }
        } else {
            TestSetup::success(
                address,
                logs,
                traces,
                AddressHashMap::default(),
                None,
                deployed_libs,
                self.fuzz_fixtures(executor, address),
                needs_setup,
            )
        };

        Ok(setup)
    }

    /// Collect fixtures from test contract.
    ///
    /// Fixtures can be defined:
    /// - as storage arrays in test contract, prefixed with `fixture`
    /// - as functions prefixed with `fixture` and followed by parameter name to
    ///   be fuzzed
    ///
    /// Storage array fixtures:
    /// `uint256[] public fixture_amount = [1, 2, 3];`
    /// define an array of uint256 values to be used for fuzzing `amount` named
    /// parameter in scope of the current test.
    ///
    /// Function fixtures:
    /// `function fixture_owner() public returns (address[] memory){}`
    /// returns an array of addresses to be used for fuzzing `owner` named
    /// parameter in scope of the current test.
    fn fuzz_fixtures(&self, executor: &mut Executor, address: Address) -> FuzzFixtures {
        let fixture_funcs = self
            .contract
            .abi
            .functions()
            .filter(|func| func.is_fixture());

        // No-op if the feature is disabled
        if !self.solidity_fuzz_fixtures {
            fixture_funcs.for_each(|func| {
                log::warn!("Possible fuzz fixture usage detected: '{}', but solidity fuzz fixtures are disabled.", &func.name);
            });

            return FuzzFixtures::default();
        };

        let mut fixtures = alloy_primitives::map::HashMap::new();
        fixture_funcs.for_each(|func| {
            if func.inputs.is_empty() {
                // Read fixtures declared as functions.
                if let Ok(CallResult {
                    raw: _,
                    decoded_result,
                }) = executor.call(CALLER, address, func, &[], U256::ZERO, None)
                {
                    fixtures.insert(fixture_name(func.name.clone()), decoded_result);
                }
            } else {
                // For reading fixtures from storage arrays we collect values by calling the
                // function with incremented indexes until there's an error.
                let mut vals = Vec::new();
                let mut index = 0;
                loop {
                    if let Ok(CallResult {
                        raw: _,
                        decoded_result,
                    }) = executor.call(
                        CALLER,
                        address,
                        func,
                        &[DynSolValue::Uint(U256::from(index), 256)],
                        U256::ZERO,
                        None,
                    ) {
                        vals.push(decoded_result);
                    } else {
                        // No result returned for this index, we reached the end of storage
                        // array or the function is not a valid fixture.
                        break;
                    }
                    index += 1;
                }
                fixtures.insert(fixture_name(func.name.clone()), DynSolValue::Array(vals));
            };
        });

        FuzzFixtures::new(fixtures)
    }

    /// Runs a single test
    ///
    /// Calls the given functions and returns the `TestResult`.
    ///
    /// State modifications are not committed to the evm database but discarded
    /// after the call, similar to `eth_call`.
    fn run_test(
        &self,
        mut executor: Executor,
        func: &Function,
        should_fail: bool,
        setup: TestSetup,
    ) -> TestResult {
        let span = info_span!("test", %should_fail);
        if !span.is_disabled() {
            let sig = &func.signature()[..];
            if enabled!(tracing::Level::TRACE) {
                span.record("sig", sig);
            } else {
                span.record("sig", sig.split('(').next().unwrap());
            }
        }
        let _guard = span.enter();

        let TestSetup {
            address,
            mut logs,
            mut traces,
            mut labeled_addresses,
            mut coverage,
            ..
        } = setup;

        // Run unit test
        let start: Instant = Instant::now();
        let (raw_call_result, reason) = match executor.execute_test(
            self.sender,
            address,
            func,
            &[],
            U256::ZERO,
            Some(self.revert_decoder),
        ) {
            Ok(res) => (res.raw, None),
            Err(EvmError::Execution(err)) => (err.raw, Some(err.reason)),
            Err(EvmError::Skip(reason)) => {
                return TestResult {
                    status: TestStatus::Skipped,
                    reason: reason.into(),
                    decoded_logs: decode_console_logs(&logs),
                    traces,
                    labeled_addresses,
                    kind: TestKind::Standard(0),
                    duration: start.elapsed(),
                    ..Default::default()
                };
            }
            Err(err) => {
                return TestResult {
                    status: TestStatus::Failure,
                    reason: Some(err.to_string()),
                    decoded_logs: decode_console_logs(&logs),
                    traces,
                    labeled_addresses,
                    kind: TestKind::Standard(0),
                    duration: start.elapsed(),
                    ..Default::default()
                };
            }
        };

        let RawCallResult {
            reverted,
            gas_used: gas,
            stipend,
            logs: execution_logs,
            traces: execution_trace,
            coverage: execution_coverage,
            labels: new_labels,
            state_changeset,
            ..
        } = raw_call_result;

        let success = executor.is_success(
            setup.address,
            reverted,
            Cow::Owned(state_changeset),
            should_fail,
        );

        labeled_addresses.extend(new_labels);
        logs.extend(execution_logs);
        HitMaps::merge_opt(&mut coverage, execution_coverage);
        traces.extend(execution_trace.map(|traces| (TraceKind::Execution, traces)));

        // Record test execution time
        let duration = start.elapsed();
        trace!(?duration, gas, reverted, should_fail, success);

        // Exclude stack trace generation from test execution time for accurate
        // reporting
        let stack_trace_result = if !success {
            let stack_trace_result: StackTraceResult =
                if let Some(indeterminism_reasons) = executor.indeterminism_reasons() {
                    indeterminism_reasons.into()
                } else {
                    self.re_run_test_for_stack_traces(func, setup.has_setup_method)
                        .into()
                };
            Some(stack_trace_result)
        } else {
            None
        };

        TestResult {
            status: match success {
                true => TestStatus::Success,
                false => TestStatus::Failure,
            },
            reason,
            counterexample: None,
            decoded_logs: decode_console_logs(&logs),
            logs,
            kind: TestKind::Standard(gas.overflowing_sub(stipend).0),
            traces,
            coverage,
            labeled_addresses,
            duration,
            gas_report_traces: Vec::new(),
            stack_trace_result,
        }
    }
    /// Re-run the deployment, setup and test execution with expensive EVM step
    /// tracing to generate a stack trace for a failed test.
    fn re_run_test_for_stack_traces(
        &self,
        func: &Function,
        needs_setup: bool,
    ) -> Result<Vec<StackTraceEntry>, StackTraceError> {
        let mut executor = self.executor_builder.clone().build();

        // We only need light-weight tracing for setup to be able to match contract
        // codes to contact addresses.
        executor.inspector.tracing(true);
        let setup = self.setup(&mut executor, needs_setup);
        if let Some(reason) = setup.reason {
            // If this function was called, the setup succeeded during test execution, so
            // this is an unexpected failure.
            return Err(StackTraceError::FailingSetup(reason));
        }

        // Collect EVM step traces that are needed for stack trace generation.
        executor.inspector.enable_for_stack_traces();

        // Run unit test
        let new_traces = match executor.execute_test(
            self.sender,
            setup.address,
            func,
            &[],
            U256::ZERO,
            Some(self.revert_decoder),
        ) {
            Ok(res) => res.raw.traces,
            Err(EvmError::Execution(err)) => err.raw.traces,
            Err(err) => return Err(err.into()),
        }
        .expect("enabled tracing");

        let mut traces = setup.traces;
        traces.push((TraceKind::Execution, new_traces));

        get_stack_trace(&*self.contract_decoder, &traces)
            .transpose()
            .expect("traces are not empty")
    }

    // It's one argument over, but follows the pattern in the file.
    #[instrument(name = "invariant_test", skip_all)]
    fn run_invariant_test(&self, args: RunInvariantTestsArgs<'_>) -> TestResult {
        let RunInvariantTestsArgs {
            executor,
            test_bytecode,
            runner,
            setup,
            invariant_config,
            func,
            call_after_invariant,
            known_contracts,
            identified_contracts,
        } = args;

        trace!(target: "edr_solidity_tests::test::fuzz", "executing invariant test for {:?}", func.name);
        let TestSetup {
            address,
            mut logs,
            mut traces,
            labeled_addresses,
            reason,
            mut coverage,
            deployed_libs,
            fuzz_fixtures,
            has_setup_method: _,
        } = setup;
        debug_assert!(reason.is_none());

        // First, run the test normally to see if it needs to be skipped.
        let start = Instant::now();
        if let Err(EvmError::Skip(reason)) = executor.call(
            self.sender,
            address,
            func,
            &[],
            U256::ZERO,
            Some(self.revert_decoder),
        ) {
            return TestResult {
                status: TestStatus::Skipped,
                reason: reason.into(),
                decoded_logs: decode_console_logs(&logs),
                labeled_addresses,
                kind: TestKind::Invariant {
                    runs: 1,
                    calls: 1,
                    reverts: 1,
                },
                coverage,
                duration: start.elapsed(),
                ..Default::default()
            };
        };

        let mut evm = InvariantExecutor::new(
            executor.clone(),
            runner,
            invariant_config.clone(),
            identified_contracts,
            known_contracts,
        );
        let invariant_contract = InvariantContract {
            address,
            invariant_function: func,
            call_after_invariant,
            abi: &self.contract.abi,
        };

        let failure_dir = invariant_config.clone().failure_dir(self.name);
        let failure_file = failure_dir
            .as_ref()
            .map(|failure_dir| failure_dir.join(&invariant_contract.invariant_function.name));

        if let Some(failure_file) = failure_file.as_ref() {
            if let Some(result) = try_to_replay_recorded_failures(ReplayRecordedFailureArgs {
                executor: executor.clone(),
                test_bytecode,
                contract_decoder: &*self.contract_decoder,
                revert_decoder: self.revert_decoder,
                failure_file,
                invariant_config: &invariant_config,
                known_contracts,
                identified_contracts,
                invariant_contract: &invariant_contract,
                logs: &mut logs,
                traces: &mut traces,
                coverage: &mut coverage,
                start: &start,
            }) {
                return result;
            }
        }

        let InvariantFuzzTestResult {
            error,
            cases,
            reverts,
            last_run_inputs,
            gas_report_traces,
            coverage: invariant_coverage,
            metrics: _metrics,
        } = match evm.invariant_fuzz(invariant_contract.clone(), &fuzz_fixtures, &deployed_libs) {
            Ok(x) => x,
            Err(e) => {
                let duration = start.elapsed();
                let stack_trace_result: StackTraceResult =
                    if let Some(indeterminism_reasons) = executor.indeterminism_reasons() {
                        indeterminism_reasons.into()
                    } else {
                        self.re_run_test_for_stack_traces(func, setup.has_setup_method)
                            .into()
                    };
                return TestResult {
                    status: TestStatus::Failure,
                    reason: Some(format!(
                        "failed to set up invariant testing environment: {e}"
                    )),
                    decoded_logs: decode_console_logs(&logs),
                    traces,
                    labeled_addresses,
                    kind: TestKind::Invariant {
                        runs: 0,
                        calls: 0,
                        reverts: 0,
                    },
                    duration,
                    stack_trace_result: Some(stack_trace_result),
                    ..Default::default()
                };
            }
        };

        HitMaps::merge_opt(&mut coverage, invariant_coverage);

        let mut counterexample = None;
        let mut stack_trace = None;
        let success = error.is_none();
        let mut reason = error.as_ref().and_then(InvariantFuzzError::revert_reason);

        match error {
            // If invariants were broken, replay the error to collect logs and traces
            Some(error) => match error {
                InvariantFuzzError::BrokenInvariant(case_data)
                | InvariantFuzzError::Revert(case_data) => {
                    // Replay error to create counterexample and to collect logs, traces and
                    // coverage.
                    match replay_error::<NestedTraceDecoderT>(ReplayErrorArgs {
                        executor: executor.clone(),
                        failed_case: &case_data,
                        invariant_contract: &invariant_contract,
                        known_contracts,
                        ided_contracts: identified_contracts.clone(),
                        logs: &mut logs,
                        traces: &mut traces,
                        coverage: &mut coverage,
                        generate_stack_trace: true,
                        contract_decoder: Some(&*self.contract_decoder),
                        revert_decoder: self.revert_decoder,
                        show_solidity: invariant_config.show_solidity,
                    }) {
                        Ok(ReplayResult {
                            counterexample_sequence,
                            stack_trace_result,
                            revert_reason,
                        }) => {
                            if !counterexample_sequence.is_empty() {
                                // Persist error in invariant failure dir.
                                if let Some(failure_dir) = failure_dir {
                                    let failure_file = failure_file
                                        .expect("failure file must be some if failure_dir is some");
                                    if let Err(err) = edr_common::fs::create_dir_all(failure_dir) {
                                        error!(%err, "Failed to create invariant failure dir");
                                    } else if let Err(err) = edr_common::fs::write_json_file(
                                        failure_file.as_path(),
                                        &InvariantPersistedFailure {
                                            call_sequence: counterexample_sequence.clone(),
                                            driver_bytecode: Some(test_bytecode.clone()),
                                        },
                                    ) {
                                        error!(%err, "Failed to record call sequence");
                                    }
                                }
                                let original_seq_len =
                                    if let TestError::Fail(_, calls) = &case_data.test_error {
                                        calls.len()
                                    } else {
                                        counterexample_sequence.len()
                                    };

                                counterexample = Some(CounterExample::Sequence(
                                    original_seq_len,
                                    counterexample_sequence,
                                ));
                            }

                            // If we can't get a revert reason for the second time, we couldn't
                            // replay the failure, so keep the original revert reason
                            // and discard the stack trace as it may be misleading.
                            if reason.is_some() && revert_reason.is_none() {
                                tracing::warn!(?invariant_contract.invariant_function, "Failed to compute stack trace");
                            } else {
                                stack_trace = stack_trace_result.map(StackTraceResult::from);
                                reason = revert_reason;
                            }
                        }
                        Err(err) => {
                            error!(%err, "Failed to replay invariant error");
                        }
                    };
                }
                InvariantFuzzError::MaxAssumeRejects(_) => {}
            },

            // If invariants ran successfully, replay the last run to collect logs and
            // traces.
            _ => {
                if let Err(err) = replay_run::<NestedTraceDecoderT>(ReplayRunArgs {
                    invariant_contract: &invariant_contract,
                    executor,
                    known_contracts,
                    ided_contracts: identified_contracts.clone(),
                    logs: &mut logs,
                    traces: &mut traces,
                    coverage: &mut coverage,
                    inputs: last_run_inputs.clone(),
                    generate_stack_trace: false,
                    contract_decoder: None,
                    revert_decoder: self.revert_decoder,
                    fail_on_revert: invariant_config.fail_on_revert,
                    show_solidity: invariant_config.show_solidity,
                }) {
                    error!(%err, "Failed to replay last invariant run");
                }
            }
        }

        TestResult {
            status: match success {
                true => TestStatus::Success,
                false => TestStatus::Failure,
            },
            reason,
            counterexample,
            decoded_logs: decode_console_logs(&logs),
            logs,
            kind: TestKind::Invariant {
                runs: cases.len(),
                calls: cases.iter().map(|sequence| sequence.cases().len()).sum(),
                reverts,
            },
            coverage,
            traces,
            labeled_addresses: labeled_addresses.clone(),
            duration: start.elapsed(),
            gas_report_traces,
            stack_trace_result: stack_trace,
        }
    }

    #[instrument(name = "fuzz_test", skip_all, fields(name = %func.signature(), %should_fail))]
    fn run_fuzz_test(
        &self,
        executor: Executor,
        func: &Function,
        should_fail: bool,
        runner: TestRunner,
        setup: TestSetup,
        fuzz_config: FuzzConfig,
    ) -> TestResult {
        let span = info_span!("fuzz_test", %should_fail);
        if !span.is_disabled() {
            let sig = &func.signature()[..];
            if enabled!(tracing::Level::TRACE) {
                span.record("test", sig);
            } else {
                span.record("test", sig.split('(').next().unwrap());
            }
        }
        let _guard = span.enter();

        let TestSetup {
            address,
            mut logs,
            mut traces,
            mut labeled_addresses,
            reason: _,
            mut coverage,
            deployed_libs,
            fuzz_fixtures,
            has_setup_method,
        } = setup;

        // Run fuzz test
        let start = Instant::now();
        let fuzzed_executor =
            FuzzedExecutor::new(executor, runner.clone(), self.sender, fuzz_config.clone());
        let result = fuzzed_executor.fuzz(
            func,
            &fuzz_fixtures,
            &deployed_libs,
            address,
            should_fail,
            self.revert_decoder,
        );

        // Check the last test result and skip the test
        // if it's marked as so.
        if let Some("SKIPPED") = result.reason.as_deref() {
            return TestResult {
                status: TestStatus::Skipped,
                reason: None,
                decoded_logs: decode_console_logs(&logs),
                traces,
                labeled_addresses,
                kind: TestKind::Standard(0),
                coverage,
                duration: start.elapsed(),
                ..Default::default()
            };
        }

        let kind = TestKind::Fuzz {
            median_gas: result.median_gas(false),
            mean_gas: result.mean_gas(false),
            runs: result.gas_by_case.len(),
        };

        // Record logs, labels and traces
        logs.extend(result.logs);
        labeled_addresses.extend(result.labeled_addresses);
        traces.extend(result.traces.map(|traces| (TraceKind::Execution, traces)));
        HitMaps::merge_opt(&mut coverage, result.coverage);

        // Record test execution time
        let duration = start.elapsed();
        trace!(?duration, success = %result.success);

        let stack_trace_result =
            if let Some(CounterExample::Single(counter_example)) = result.counterexample.as_ref() {
                let stack_trace_result: StackTraceResult = if let Some(indeterminism_reasons) =
                    counter_example.indeterminism_reasons.clone()
                {
                    indeterminism_reasons.into()
                } else {
                    self.re_run_fuzz_counterexample_for_stack_traces(
                        address,
                        counter_example,
                        has_setup_method,
                    )
                    .into()
                };
                Some(stack_trace_result)
            } else {
                None
            };

        TestResult {
            status: match result.success {
                true => TestStatus::Success,
                false => TestStatus::Failure,
            },
            reason: result.reason,
            counterexample: result.counterexample,
            decoded_logs: decode_console_logs(&logs),
            logs,
            kind,
            traces,
            coverage,
            labeled_addresses,
            duration,
            gas_report_traces: result
                .gas_report_traces
                .into_iter()
                .map(|t| vec![t])
                .collect(),
            stack_trace_result,
        }
    }

    /// Re-run the deployment, setup and test execution with expensive EVM step
    /// tracing to generate a stack trace for a fuzz counterexample.
    fn re_run_fuzz_counterexample_for_stack_traces(
        &self,
        address: Address,
        counter_example: &BaseCounterExample,
        needs_setup: bool,
    ) -> Result<Vec<StackTraceEntry>, StackTraceError> {
        let mut executor = self.executor_builder.clone().build();

        // We only need light-weight tracing for setup to be able to match contract
        // codes to contact addresses.
        executor.inspector.tracing(true);
        let setup = self.setup(&mut executor, needs_setup);
        if let Some(reason) = setup.reason {
            // If this function was called, the setup succeeded during test execution, so
            // this is an unexpected failure.
            return Err(StackTraceError::FailingSetup(reason));
        }

        // Collect EVM step traces that are needed for stack trace generation.
        executor.inspector.enable_for_stack_traces();

        // Run counterexample test
        let (call, _cow_backend) = executor
            .call_raw(
                self.sender,
                address,
                counter_example.calldata.clone(),
                U256::ZERO,
            )
            .map_err(|err| StackTraceError::Evm(err.to_string()))?;

        let mut traces = setup.traces;
        traces.push((TraceKind::Execution, call.traces.expect("tracing is on")));

        get_stack_trace(&*self.contract_decoder, &traces)
            .transpose()
            .expect("traces are not empty")
    }
}

struct RunInvariantTestsArgs<'a> {
    executor: Executor,
    test_bytecode: &'a Bytes,
    runner: TestRunner,
    setup: TestSetup,
    invariant_config: InvariantConfig,
    func: &'a Function,
    call_after_invariant: bool,
    known_contracts: &'a ContractsByArtifact,
    identified_contracts: &'a ContractsByAddress,
}

/// Holds data about a persisted invariant failure.
#[derive(Serialize, Deserialize)]
struct InvariantPersistedFailure {
    /// Recorded counterexample.
    call_sequence: Vec<BaseCounterExample>,
    /// Bytecode of the test contract that generated the counterexample.
    #[serde(skip_serializing_if = "Option::is_none")]
    driver_bytecode: Option<Bytes>,
}

/// Helper function to load failed call sequence from file.
/// Ignores failure if generated with different test contract than the current
/// one.
fn persisted_call_sequence(path: &Path, bytecode: &Bytes) -> Option<Vec<BaseCounterExample>> {
    edr_common::fs::read_json_file::<InvariantPersistedFailure>(path).ok().and_then(
        |persisted_failure| {
            if let Some(persisted_bytecode) = &persisted_failure.driver_bytecode {
                // Ignore persisted sequence if test bytecode doesn't match.
                if !bytecode.eq(persisted_bytecode) {
                    tracing::warn!("\
                            Failure from {:?} file was ignored because test contract bytecode has changed.",
                        path
                    );
                    return None;
                }
            };
            Some(persisted_failure.call_sequence)
        },
    )
}

struct ReplayRecordedFailureArgs<'a, NestedTraceDecoderT: NestedTraceDecoder> {
    executor: Executor,
    test_bytecode: &'a Bytes,
    contract_decoder: &'a NestedTraceDecoderT,
    revert_decoder: &'a RevertDecoder,
    failure_file: &'a Path,
    invariant_config: &'a InvariantConfig,
    known_contracts: &'a ContractsByArtifact,
    identified_contracts: &'a ContractsByAddress,
    invariant_contract: &'a InvariantContract<'a>,
    logs: &'a mut Vec<Log>,
    traces: &'a mut Traces,
    coverage: &'a mut Option<HitMaps>,
    start: &'a Instant,
}

fn try_to_replay_recorded_failures<NestedTraceDecoderT: NestedTraceDecoder>(
    args: ReplayRecordedFailureArgs<'_, NestedTraceDecoderT>,
) -> Option<TestResult> {
    let ReplayRecordedFailureArgs {
        executor,
        test_bytecode,
        contract_decoder,
        revert_decoder,
        failure_file,
        invariant_config,
        known_contracts,
        identified_contracts,
        invariant_contract,
        logs,
        traces,
        coverage,
        start,
    } = args;

    if let Some(call_sequence) = persisted_call_sequence(failure_file, test_bytecode) {
        // Create calls from failed sequence and check if invariant still broken.
        let txes = call_sequence
            .clone()
            .into_iter()
            .map(|seq| BasicTxDetails {
                sender: seq.sender.unwrap_or_default(),
                call_details: CallDetails {
                    target: seq.addr.unwrap_or_default(),
                    calldata: seq.calldata,
                },
            })
            .collect::<Vec<BasicTxDetails>>();
        if let Ok((success, replayed_entirely)) = check_sequence(
            executor.clone(),
            &txes,
            (0..min(txes.len(), invariant_config.depth as usize)).collect(),
            invariant_contract.address,
            invariant_contract
                .invariant_function
                .selector()
                .to_vec()
                .into(),
            invariant_config.fail_on_revert,
            invariant_contract.call_after_invariant,
        ) {
            if !success {
                // If sequence still fails then replay error to collect traces and
                // exit without executing new runs.
                let stack_trace_result = replay_run(ReplayRunArgs {
                    invariant_contract,
                    executor,
                    known_contracts,
                    ided_contracts: identified_contracts.clone(),
                    logs,
                    traces,
                    coverage,
                    inputs: txes,
                    generate_stack_trace: true,
                    contract_decoder: Some(contract_decoder),
                    revert_decoder,
                    fail_on_revert: invariant_config.fail_on_revert,
                    show_solidity: invariant_config.show_solidity,
                })
                .map_or(None, |result| {
                    result.stack_trace_result.map(StackTraceResult::from)
                });
                let reason = if replayed_entirely {
                    Some(format!(
                        "{} replay failure",
                        invariant_contract.invariant_function.name
                    ))
                } else {
                    Some(format!(
                        "{} persisted failure revert",
                        invariant_contract.invariant_function.name
                    ))
                };

                return Some(TestResult {
                    status: TestStatus::Failure,
                    reason,
                    decoded_logs: decode_console_logs(logs),
                    traces: traces.clone(),
                    gas_report_traces: vec![],
                    coverage: coverage.clone(),
                    counterexample: Some(CounterExample::Sequence(
                        call_sequence.len(),
                        call_sequence,
                    )),
                    kind: TestKind::Invariant {
                        runs: 1,
                        calls: 1,
                        reverts: 1,
                    },
                    duration: start.elapsed(),
                    logs: vec![],
                    labeled_addresses: AddressHashMap::<String>::default(),
                    stack_trace_result,
                });
            }
        }
    }
    None
}

/// Returns `true` if the function is a test function that matches the given
/// filter.
fn is_matching_test(func: &Function, filter: &dyn TestFilter) -> bool {
    func.is_any_test() && filter.matches_test(&func.signature())
}
