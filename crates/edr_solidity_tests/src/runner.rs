//! The Forge test runner.
use std::{
    borrow::Cow, cmp::min, collections::BTreeMap, marker::PhantomData, path::Path, sync::Arc,
    time::Instant,
};

use alloy_dyn_abi::{DynSolValue, JsonAbiExt};
use alloy_json_abi::Function;
use alloy_primitives::{Address, Bytes, U256};
use derive_where::derive_where;
use edr_chain_spec::{EvmHaltReason, HaltReasonTrait};
use edr_solidity::{
    contract_decoder::SyncNestedTraceDecoder, solidity_stack_trace::StackTraceEntry,
};
use eyre::Result;
use foundry_evm::{
    abi::{TestFunctionExt, TestFunctionKind},
    constants::{CALLER, LIBRARY_DEPLOYER},
    contracts::{ContractsByAddress, ContractsByArtifact},
    decode::RevertDecoder,
    evm_context::{
        BlockEnvTr, ChainContextTr, EvmBuilderTrait, HardforkTr, TransactionEnvTr,
        TransactionErrorTrait,
    },
    executors::{
        fuzz::FuzzedExecutor,
        invariant::{
            check_sequence, replay_error, replay_run, InvariantConfig, InvariantExecutor,
            InvariantFuzzError, ReplayErrorArgs, ReplayResult, ReplayRunArgs,
        },
        stack_trace::{get_stack_trace, StackTraceError, StackTraceResult},
        CallResult, EvmError, Executor, ExecutorBuilder, ITest, RawCallResult,
    },
    fuzz::{
        fixture_name,
        invariant::{CallDetails, InvariantContract},
        CounterExample, FuzzFixtures,
    },
    traces::{load_contracts, TraceKind, TracingMode},
};
use itertools::Itertools;
use proptest::test_runner::{FailurePersistence, RngAlgorithm, TestError, TestRng, TestRunner};
use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use tracing::Span;

use crate::{
    error::TestRunnerError,
    fuzz::{invariant::BasicTxDetails, BaseCounterExample, FuzzConfig},
    multi_runner::TestContract,
    result::{SuiteResult, TestResult, TestSetup},
    revm::context::result::HaltReason,
    TestFilter,
};

/// A type that executes all tests of a contract
#[derive_where(Clone, Debug; BlockT, TxT, ChainContextT, HardforkT, NestedTraceDecoderT)]
pub struct ContractRunner<
    'a,
    BlockT: BlockEnvTr,
    ChainContextT: ChainContextTr,
    EvmBuilderT: EvmBuilderTrait<BlockT, ChainContextT, HaltReasonT, HardforkT, TransactionErrorT, TxT>,
    HaltReasonT: HaltReasonTrait,
    HardforkT: HardforkTr,
    TransactionErrorT: TransactionErrorTrait,
    TxT: TransactionEnvTr,
    NestedTraceDecoderT,
> {
    /// The name of the contract.
    name: &'a str,
    /// The data of the contract being ran.
    contract: &'a TestContract,
    /// Revert decoder. Contains all known errors.
    revert_decoder: &'a RevertDecoder,
    /// Known contracts by artifact id
    known_contracts: &'a ContractsByArtifact,
    /// Libraries to deploy.
    libs_to_deploy: &'a [Bytes],
    /// Provides contract metadata for calldata and traces.
    contract_decoder: Arc<NestedTraceDecoderT>,
    /// The initial balance of the test contract
    initial_balance: U256,
    /// The address which will be used as the `from` field in all EVM calls
    sender: Address,
    /// Whether to enable solidity fuzz fixtures support
    solidity_fuzz_fixtures: bool,
    /// Fuzz config
    fuzz_config: &'a FuzzConfig,
    /// Invariant config
    invariant_config: &'a InvariantConfig,
    /// Whether to enable table tests
    enable_table_tests: bool,
    /// The config values required to build the executor.
    executor_builder: ExecutorBuilder<BlockT, TxT, HardforkT, ChainContextT>,
    /// The span of the contract.
    span: tracing::Span,

    #[allow(clippy::type_complexity)]
    _phantom: PhantomData<fn() -> (EvmBuilderT, HaltReasonT, TransactionErrorT)>,
}

/// Options for [`ContractRunner`].
#[derive(Clone, Debug)]
pub struct ContractRunnerOptions<'a> {
    /// The initial balance of the test contract
    pub initial_balance: U256,
    /// The address which will be used as the `from` field in all EVM calls
    pub sender: Address,
    /// whether to enable solidity fuzz fixtures support
    pub enable_fuzz_fixtures: bool,
    /// Whether to enable table test support
    pub enable_table_tests: bool,
    /// Fuzz config
    pub fuzz_config: &'a FuzzConfig,
    /// Invariant config
    pub invariant_config: &'a InvariantConfig,
}

/// Contract artifact related arguments to the contract runner.
pub struct ContractRunnerArtifacts<
    'a,
    HaltReasonT: HaltReasonTrait,
    NestedTracerDecoderT: SyncNestedTraceDecoder<HaltReasonT>,
> {
    pub revert_decoder: &'a RevertDecoder,
    pub known_contracts: &'a ContractsByArtifact,
    pub libs_to_deploy: &'a [Bytes],
    pub contract_decoder: Arc<NestedTracerDecoderT>,
    pub _phantom: PhantomData<HaltReasonT>,
}

impl<
        'a,
        BlockT: BlockEnvTr,
        ChainContextT: ChainContextTr,
        EvmBuilderT: EvmBuilderTrait<BlockT, ChainContextT, HaltReasonT, HardforkT, TransactionErrorT, TxT>,
        HaltReasonT: HaltReasonTrait,
        HardforkT: HardforkTr,
        NestedTraceDecoderT: SyncNestedTraceDecoder<HaltReasonT>,
        TransactionErrorT: TransactionErrorTrait,
        TxT: TransactionEnvTr,
    >
    ContractRunner<
        'a,
        BlockT,
        ChainContextT,
        EvmBuilderT,
        HaltReasonT,
        HardforkT,
        TransactionErrorT,
        TxT,
        NestedTraceDecoderT,
    >
{
    pub fn new(
        name: &'a str,
        executor_builder: ExecutorBuilder<BlockT, TxT, HardforkT, ChainContextT>,
        contract: &'a TestContract,
        artifacts: ContractRunnerArtifacts<'a, HaltReasonT, NestedTraceDecoderT>,
        options: ContractRunnerOptions<'a>,
        span: Span,
    ) -> Self {
        let ContractRunnerArtifacts {
            revert_decoder,
            known_contracts,
            libs_to_deploy,
            contract_decoder,
            _phantom: _,
        } = artifacts;
        let ContractRunnerOptions {
            initial_balance,
            sender,
            enable_fuzz_fixtures: solidity_fuzz_fixtures,
            enable_table_tests,
            fuzz_config,
            invariant_config,
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
            solidity_fuzz_fixtures,
            enable_table_tests,
            fuzz_config,
            invariant_config,
            executor_builder,
            span,
            _phantom: PhantomData,
        }
    }
}

impl<
        BlockT: BlockEnvTr,
        ChainContextT: 'static + ChainContextTr,
        EvmBuilderT: 'static
            + EvmBuilderTrait<BlockT, ChainContextT, HaltReasonT, HardforkT, TransactionErrorT, TxT>,
        HaltReasonT: 'static + HaltReasonTrait + TryInto<EvmHaltReason>,
        HardforkT: HardforkTr,
        TransactionErrorT: TransactionErrorTrait,
        TxT: TransactionEnvTr,
        NestedTraceDecoderT: SyncNestedTraceDecoder<HaltReasonT>,
    >
    ContractRunner<
        '_,
        BlockT,
        ChainContextT,
        EvmBuilderT,
        HaltReasonT,
        HardforkT,
        TransactionErrorT,
        TxT,
        NestedTraceDecoderT,
    >
{
    /// Deploys the test contract inside the runner from the sending account,
    /// and optionally runs the `setUp` function on the test contract.
    fn setup(
        &self,
        executor: &mut Executor<
            BlockT,
            TxT,
            EvmBuilderT,
            HaltReasonT,
            HardforkT,
            TransactionErrorT,
            ChainContextT,
        >,
        needs_setup: bool,
    ) -> TestSetup<HaltReasonT> {
        self._setup(executor, needs_setup).unwrap_or_else(|err| {
            if err.to_string().contains("skipped") {
                TestSetup::skipped(err.to_string())
            } else {
                TestSetup::failed(err.to_string())
            }
        })
    }

    fn _setup(
        &self,
        executor: &mut Executor<
            BlockT,
            TxT,
            EvmBuilderT,
            HaltReasonT,
            HardforkT,
            TransactionErrorT,
            ChainContextT,
        >,
        call_setup: bool,
    ) -> Result<TestSetup<HaltReasonT>> {
        trace!(?call_setup, "setting up");

        // We max out their balance so that they can deploy and make calls.
        executor.set_balance(self.sender, U256::MAX)?;
        executor.set_balance(CALLER, U256::MAX)?;

        // We set the nonce of the deployer accounts to 1 to get the same addresses as
        // DappTools
        executor.set_nonce(self.sender, 1)?;

        // Deploy libraries
        executor.set_balance(LIBRARY_DEPLOYER, U256::MAX)?;

        let mut result = TestSetup {
            has_setup_method: call_setup,
            ..Default::default()
        };
        for code in self.libs_to_deploy.iter() {
            let deploy_result = executor.deploy(
                LIBRARY_DEPLOYER,
                code.clone(),
                U256::ZERO,
                Some(self.revert_decoder),
            );

            // Record deployed library address.
            if let Ok(deployed) = &deploy_result {
                result.deployed_libs.push(deployed.address);
            }

            let (raw, reason) = RawCallResult::from_evm_result(deploy_result.map(Into::into))?;
            result.extend(raw, TraceKind::Deployment);
            if reason.is_some() {
                result.reason = reason;
                return Ok(result);
            }
        }

        let address = self.sender.create(executor.get_nonce(self.sender)?);
        result.address = address;

        // Set the contracts initial balance before deployment, so it is available
        // during construction
        executor.set_balance(address, self.initial_balance)?;

        // Deploy the test contract
        let deploy_result = executor.deploy(
            self.sender,
            self.contract.bytecode.clone(),
            U256::ZERO,
            Some(self.revert_decoder),
        );

        result.deployment_failure = deploy_result.is_err();

        if let Ok(dr) = &deploy_result {
            debug_assert_eq!(dr.address, address);
        }
        let (raw, reason) = RawCallResult::from_evm_result(deploy_result.map(Into::into))?;
        result.extend(raw, TraceKind::Deployment);
        if reason.is_some() {
            result.reason = reason;
            return Ok(result);
        }

        // Reset `self.sender`s and `CALLER`s balance to the initial balance we want
        executor.set_balance(self.sender, self.initial_balance)?;
        executor.set_balance(CALLER, self.initial_balance)?;
        executor.set_balance(LIBRARY_DEPLOYER, self.initial_balance)?;

        executor.deploy_create2_deployer()?;

        // Optionally call the `setUp` function
        if call_setup {
            trace!("calling setUp");
            let res = executor.setup(None, address, Some(self.revert_decoder));
            let (raw, reason) = RawCallResult::from_evm_result(res)?;
            result.extend(raw, TraceKind::Setup);
            result.reason = reason;
        }

        if self.solidity_fuzz_fixtures {
            result.fuzz_fixtures = self.fuzz_fixtures(executor, address);
        }

        Ok(result)
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
    fn fuzz_fixtures(
        &self,
        executor: &Executor<
            BlockT,
            TxT,
            EvmBuilderT,
            HaltReasonT,
            HardforkT,
            TransactionErrorT,
            ChainContextT,
        >,
        address: Address,
    ) -> FuzzFixtures {
        let mut fixtures = alloy_primitives::map::HashMap::default();
        let fixture_functions = self
            .contract
            .abi
            .functions()
            .filter(|func| func.is_fixture());
        for func in fixture_functions {
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
        }
        FuzzFixtures::new(fixtures)
    }
}

impl<
        BlockT: BlockEnvTr,
        ChainContextT: 'static + ChainContextTr + Send + Sync,
        EvmBuilderT: 'static
            + EvmBuilderTrait<BlockT, ChainContextT, HaltReasonT, HardforkT, TransactionErrorT, TxT>,
        HaltReasonT: 'static + HaltReasonTrait + TryInto<EvmHaltReason> + Send + Sync,
        HardforkT: HardforkTr,
        TransactionErrorT: TransactionErrorTrait,
        TxT: TransactionEnvTr,
        NestedTraceDecoderT: SyncNestedTraceDecoder<HaltReasonT>,
    >
    ContractRunner<
        '_,
        BlockT,
        ChainContextT,
        EvmBuilderT,
        HaltReasonT,
        HardforkT,
        TransactionErrorT,
        TxT,
        NestedTraceDecoderT,
    >
{
    /// Runs all tests for a contract whose names match the provided regular
    /// expression
    pub fn run_tests(
        self,
        filter: &dyn TestFilter,
        tokio_handle: &tokio::runtime::Handle,
    ) -> Result<SuiteResult<HaltReasonT>, TestRunnerError> {
        // Forge doesn't include building the executor in the test time, so we're
        // excluding it as well.
        let mut executor = self.executor_builder.clone().build()?;

        let start = Instant::now();
        let mut warnings = Vec::new();

        let setup_fns: Vec<_> = self
            .contract
            .abi
            .functions()
            .filter(|func| func.name.is_setup())
            .collect();

        let call_setup = setup_fns.len() == 1
            && setup_fns
                .first()
                .expect("setup_fns has exactly one element")
                .name
                == "setUp";

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
            return Ok(SuiteResult::new(
                start.elapsed(),
                [(
                    "setUp()".to_string(),
                    TestResult::fail("multiple setUp functions".to_string()),
                )]
                .into(),
                warnings,
            ));
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
            return Ok(SuiteResult::new(
                start.elapsed(),
                [(
                    "afterInvariant()".to_string(),
                    TestResult::fail("multiple afterInvariant functions".to_string()),
                )]
                .into(),
                warnings,
            ));
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

        // Invariant testing requires tracing to figure out what contracts were created.
        // We also want to disable `debug` for setup since we won't be using those
        // traces.
        let has_invariants = self
            .contract
            .abi
            .functions()
            .any(foundry_evm::abi::TestFunctionExt::is_invariant_test);

        let prev_tracer = executor.inspector_mut().tracer.take();
        if prev_tracer.is_some() || has_invariants {
            executor.set_tracing(TracingMode::WithoutSteps);
        }

        let setup_time = Instant::now();
        let mut setup = self.setup(&mut executor, call_setup);
        debug!("finished setting up in {:?}", setup_time.elapsed());

        executor.inspector_mut().tracer = prev_tracer;

        if setup.reason.is_some() {
            // We want to report execution time without stack trace generation as people use
            // these numbers to reason about the performance of their code.
            let elapsed = start.elapsed();

            setup.stack_trace_result = if executor.tracer_records_steps() {
                // We collected steps during setup, so we can generate the stack trace
                get_stack_trace(&*self.contract_decoder, &setup.traces)
                    .transpose()
                    .map(Into::into)
            } else if let Some(indeterminism_reasons) = setup.indeterminism_reasons.as_ref() {
                // We cannot re-run the setup due to indeterminism, so we return the
                // indeterminism reasons
                Some(indeterminism_reasons.clone().into())
            } else {
                // Re-execute with collection of steps to generate stack traces
                let mut executor = self.executor_builder.clone().build()?;
                executor.set_tracing(TracingMode::WithSteps);
                let setup_for_stack_traces = self.setup(&mut executor, call_setup);

                get_stack_trace(&*self.contract_decoder, &setup_for_stack_traces.traces)
                    .transpose()
                    .map(Into::into)
            };

            // The setup failed, so we return a single test result for `setUp`
            let fail_msg = if !setup.deployment_failure {
                "setUp()".to_string()
            } else {
                "constructor()".to_string()
            };
            return Ok(SuiteResult::new(
                elapsed,
                [(fail_msg, TestResult::setup_result(setup))].into(),
                warnings,
            ));
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

        let test_fail_functions = functions
            .iter()
            .filter(|func| func.test_function_kind().is_any_test_fail());
        if test_fail_functions.clone().next().is_some() {
            let fail = || {
                TestResult::fail("`testFail*` has been removed. Consider changing to test_Revert[If|When]_Condition and expecting a revert".to_string())
            };
            let test_results = test_fail_functions
                .map(|func| (func.signature(), fail()))
                .collect();
            return Ok(SuiteResult::new(start.elapsed(), test_results, warnings));
        }

        let test_results = functions
            .par_iter()
            .map(|&func| {
                let _tokio_guard = tokio_handle.enter();

                let _span_guard;
                let current_span = tracing::Span::current();
                if current_span.is_none() || current_span.id() != self.span.id() {
                    _span_guard = self.span.enter();
                }

                let sig = func.signature();
                let kind = func.test_function_kind();

                let _guard = debug_span!(
                    "test",
                    %kind,
                    name = %if enabled!(tracing::Level::TRACE) { &sig } else { &func.name },
                )
                .entered();

                let res = FunctionRunner::new(&self, &executor, &setup).run(
                    func,
                    kind,
                    call_after_invariant,
                    identified_contracts.as_ref(),
                );

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
        Ok(suite_result)
    }
}

/// Executes a single test function, returning a [`TestResult`].
struct FunctionRunner<
    'a,
    BlockT: BlockEnvTr,
    ChainContextT: ChainContextTr,
    EvmBuilderT: EvmBuilderTrait<BlockT, ChainContextT, HaltReasonT, HardforkT, TransactionErrorT, TxT>,
    HaltReasonT: HaltReasonTrait,
    HardforkT: HardforkTr,
    NestedTraceDecoderT,
    TransactionErrorT: TransactionErrorTrait,
    TxT: TransactionEnvTr,
> {
    /// The EVM executor.
    executor: Cow<
        'a,
        Executor<
            BlockT,
            TxT,
            EvmBuilderT,
            HaltReasonT,
            HardforkT,
            TransactionErrorT,
            ChainContextT,
        >,
    >,
    /// The parent runner.
    cr: &'a ContractRunner<
        'a,
        BlockT,
        ChainContextT,
        EvmBuilderT,
        HaltReasonT,
        HardforkT,
        TransactionErrorT,
        TxT,
        NestedTraceDecoderT,
    >,
    /// The test setup result.
    setup: &'a TestSetup<HaltReasonT>,
    /// The test result. Returned after running the test.
    result: TestResult<HaltReasonT>,
}

impl<
        'a,
        BlockT: BlockEnvTr,
        ChainContextT: 'static + ChainContextTr,
        EvmBuilderT: 'static
            + EvmBuilderTrait<BlockT, ChainContextT, HaltReasonT, HardforkT, TransactionErrorT, TxT>,
        HaltReasonT: 'static + HaltReasonTrait + TryInto<HaltReason>,
        HardforkT: HardforkTr,
        NestedTraceDecoderT: SyncNestedTraceDecoder<HaltReasonT>,
        TransactionErrorT: TransactionErrorTrait,
        TxT: TransactionEnvTr,
    >
    FunctionRunner<
        'a,
        BlockT,
        ChainContextT,
        EvmBuilderT,
        HaltReasonT,
        HardforkT,
        NestedTraceDecoderT,
        TransactionErrorT,
        TxT,
    >
{
    fn new(
        cr: &'a ContractRunner<
            'a,
            BlockT,
            ChainContextT,
            EvmBuilderT,
            HaltReasonT,
            HardforkT,
            TransactionErrorT,
            TxT,
            NestedTraceDecoderT,
        >,
        executor: &'a Executor<
            BlockT,
            TxT,
            EvmBuilderT,
            HaltReasonT,
            HardforkT,
            TransactionErrorT,
            ChainContextT,
        >,
        setup: &'a TestSetup<HaltReasonT>,
    ) -> Self {
        Self {
            executor: Cow::Borrowed(executor),
            cr,
            setup,
            result: TestResult::new(setup),
        }
    }

    fn run(
        self,
        func: &Function,
        kind: TestFunctionKind,
        call_after_invariant: bool,
        identified_contracts: Option<&ContractsByAddress>,
    ) -> TestResult<HaltReasonT> {
        match kind {
            TestFunctionKind::UnitTest { .. } => self.run_unit_test(func),
            TestFunctionKind::FuzzTest { .. } => self.run_fuzz_test(func),
            TestFunctionKind::TableTest => self.run_table_test(func),
            TestFunctionKind::InvariantTest => {
                let test_bytecode = &self.cr.contract.bytecode;
                self.run_invariant_test(
                    func,
                    call_after_invariant,
                    identified_contracts.expect("must be set if there are invariant tests"),
                    test_bytecode,
                )
            }
            _ => unreachable!(),
        }
    }

    /// Runs a single unit test.
    ///
    /// Applies before test txes (if any), runs current test and returns the
    /// `TestResult`.
    ///
    /// Before test txes are applied in order and state modifications committed
    /// to the EVM database (therefore the unit test call will be made on
    /// modified state). State modifications of before test txes and unit
    /// test function call are discarded after test ends, similar to
    /// `eth_call`.
    fn run_unit_test(mut self, func: &Function) -> TestResult<HaltReasonT> {
        let start: Instant = Instant::now();

        // Prepare unit test execution.
        if self.prepare_test(func, start).is_err() {
            return self.result;
        }

        // Run current unit test.
        let (mut raw_call_result, reason) = match self.executor.call(
            self.cr.sender,
            self.setup.address,
            func,
            &[],
            U256::ZERO,
            Some(self.cr.revert_decoder),
        ) {
            Ok(res) => (res.raw, None),
            Err(EvmError::Execution(err)) => (err.raw, Some(err.reason)),
            Err(EvmError::Skip(reason)) => {
                self.result.single_skip(reason);
                return self.result;
            }
            Err(err) => {
                self.result
                    .single_fail(Some(err.to_string()), start.elapsed());
                return self.result;
            }
        };

        let success = self
            .executor
            .is_raw_call_mut_success(&mut raw_call_result, false);

        let elapsed = start.elapsed();

        // Exclude stack trace generation from test execution time for accurate
        // reporting
        self.result.stack_trace_result = if !success {
            let stack_trace_result: StackTraceResult<HaltReasonT> =
                if self.executor.tracer_records_steps() {
                    get_stack_trace(&*self.cr.contract_decoder, &self.result.traces)
                        .transpose()
                        .expect("traces are not empty")
                        .into()
                } else if let Some(indeterminism_reasons) =
                    raw_call_result.indeterminism_reasons.take()
                {
                    indeterminism_reasons.into()
                } else {
                    self.re_run_test_for_stack_traces(func, &[], self.setup.has_setup_method)
                        .into()
                };
            Some(stack_trace_result)
        } else {
            None
        };

        self.result
            .single_result(success, reason, raw_call_result, elapsed);

        self.result
    }

    /// Runs a table test.
    /// The parameters dataset (table) is created from defined parameter
    /// fixtures, therefore each test table parameter should have the same
    /// number of fixtures defined. E.g. for table test
    /// - `table_test(uint256 amount, bool swap)` fixtures are defined as
    /// - `uint256[] public fixtureAmount = [2, 5]`
    /// - `bool[] public fixtureSwap = [true, false]` The `table_test` is then
    ///   called with the pair of args `(2, true)` and `(5, false)`.
    fn run_table_test(mut self, func: &Function) -> TestResult<HaltReasonT> {
        let start = Instant::now();

        if !self.cr.enable_table_tests {
            self.result.single_fail(
                Some("Table tests are not supported".into()),
                start.elapsed(),
            );
            return self.result;
        };

        // Prepare unit test execution.
        if self.prepare_test(func, start).is_err() {
            return self.result;
        }

        // Extract and validate fixtures for the first table test parameter.
        let Some(first_param) = func.inputs.first() else {
            self.result.single_fail(
                Some("Table test should have at least one parameter".into()),
                start.elapsed(),
            );
            return self.result;
        };

        let Some(first_param_fixtures) =
            &self.setup.fuzz_fixtures.param_fixtures(first_param.name())
        else {
            self.result.single_fail(
                Some("Table test should have fixtures defined".into()),
                start.elapsed(),
            );
            return self.result;
        };

        if first_param_fixtures.is_empty() {
            self.result.single_fail(
                Some("Table test should have at least one fixture".into()),
                start.elapsed(),
            );
            return self.result;
        }

        let fixtures_len = first_param_fixtures.len();
        let mut table_fixtures = vec![&first_param_fixtures[..]];

        // Collect fixtures for remaining parameters.
        for param in func.inputs.get(1..).unwrap_or(&[]) {
            let param_name = param.name();
            let Some(fixtures) = &self.setup.fuzz_fixtures.param_fixtures(param.name()) else {
                self.result.single_fail(
                    Some(format!("No fixture defined for param {param_name}")),
                    start.elapsed(),
                );
                return self.result;
            };

            if fixtures.len() != fixtures_len {
                self.result.single_fail(
                    Some(format!(
                        "{} fixtures defined for {param_name} (expected {})",
                        fixtures.len(),
                        fixtures_len
                    )),
                    start.elapsed(),
                );
                return self.result;
            }

            table_fixtures.push(&fixtures[..]);
        }

        for i in 0..fixtures_len {
            // Increment progress bar.
            let args = table_fixtures
                .iter()
                .filter_map(|row| row.get(i).cloned())
                .collect_vec();
            let (mut raw_call_result, reason) = match self.executor.call(
                self.cr.sender,
                self.setup.address,
                func,
                &args,
                U256::ZERO,
                Some(self.cr.revert_decoder),
            ) {
                Ok(res) => (res.raw, None),
                Err(EvmError::Execution(err)) => (err.raw, Some(err.reason)),
                Err(EvmError::Skip(reason)) => {
                    self.result.single_skip(reason);
                    return self.result;
                }
                Err(err) => {
                    self.result
                        .single_fail(Some(err.to_string()), start.elapsed());
                    return self.result;
                }
            };

            let is_success = self
                .executor
                .is_raw_call_mut_success(&mut raw_call_result, false);
            // Record counterexample if test fails.
            if !is_success {
                self.result.counterexample =
                    Some(CounterExample::Single(BaseCounterExample::from_fuzz_call(
                        Bytes::from(
                            func.abi_encode_input(&args)
                                .expect("args have valid abi encoding"),
                        ),
                        &args,
                        raw_call_result.traces.clone(),
                        raw_call_result.indeterminism_reasons.clone(),
                    )));
                let elapsed = start.elapsed();

                let stack_trace_result: StackTraceResult<HaltReasonT> =
                    if self.executor.tracer_records_steps() {
                        get_stack_trace(&*self.cr.contract_decoder, &self.result.traces)
                            .transpose()
                            .expect("traces are not empty")
                            .into()
                    } else if let Some(indeterminism_reasons) =
                        raw_call_result.indeterminism_reasons.take()
                    {
                        indeterminism_reasons.into()
                    } else {
                        self.re_run_test_for_stack_traces(func, &args, self.setup.has_setup_method)
                            .into()
                    };
                self.result.stack_trace_result = Some(stack_trace_result);

                self.result
                    .single_result(false, reason, raw_call_result, elapsed);

                return self.result;
            }

            // If it's the last iteration and all other runs succeeded, then use last call
            // result for logs and traces.
            if i == fixtures_len - 1 {
                self.result
                    .single_result(true, None, raw_call_result, start.elapsed());
                return self.result;
            }
        }

        self.result
    }

    fn run_invariant_test(
        mut self,
        func: &Function,
        call_after_invariant: bool,
        identified_contracts: &ContractsByAddress,
        test_bytecode: &Bytes,
    ) -> TestResult<HaltReasonT> {
        let start = Instant::now();

        // First, run the test normally to see if it needs to be skipped.
        if let Err(EvmError::Skip(reason)) = self.executor.call(
            self.cr.sender,
            self.setup.address,
            func,
            &[],
            U256::ZERO,
            Some(self.cr.revert_decoder),
        ) {
            self.result.invariant_skip(reason, start.elapsed());
            return self.result;
        };

        let runner = self.invariant_runner();
        let invariant_config = self.cr.invariant_config;

        let mut executor = self.clone_executor();
        // Enable edge coverage if running with coverage guided fuzzing or with edge
        // coverage metrics (useful for benchmarking the fuzzer).
        executor.inspector_mut().collect_edge_coverage(
            invariant_config.corpus_dir.is_some() || invariant_config.show_edge_coverage,
        );

        let mut evm = InvariantExecutor::new(
            executor,
            runner,
            invariant_config.clone(),
            identified_contracts,
            self.cr.known_contracts,
        );
        let invariant_contract = InvariantContract {
            address: self.setup.address,
            invariant_function: func,
            call_after_invariant,
            abi: &self.cr.contract.abi,
        };

        let failure_dir = invariant_config.clone().failure_dir(self.cr.name);
        let failure_file = failure_dir
            .as_ref()
            .map(|failure_dir| failure_dir.join(&invariant_contract.invariant_function.name));

        // Try to replay recorded failure if any.
        if let Some(failure_file) = failure_file.as_ref()
            && let Some(mut call_sequence) =
                persisted_call_sequence(failure_file.as_path(), test_bytecode)
        {
            // Create calls from failed sequence and check if invariant still broken.
            let txes = call_sequence
                .iter_mut()
                .map(|seq| BasicTxDetails {
                    sender: seq.sender.unwrap_or_default(),
                    call_details: CallDetails {
                        target: seq.addr.unwrap_or_default(),
                        calldata: seq.calldata.clone(),
                    },
                })
                .collect::<Vec<BasicTxDetails>>();
            if let Ok((success, replayed_entirely)) = check_sequence(
                self.clone_executor(),
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
            ) && !success
            {
                // If sequence still fails then replay error to collect traces and
                // exit without executing new runs.
                let stack_trace_result = replay_run(ReplayRunArgs {
                    executor: self.clone_executor(),
                    invariant_contract: &invariant_contract,
                    known_contracts: self.cr.known_contracts,
                    ided_contracts: identified_contracts.clone(),
                    logs: &mut self.result.logs,
                    traces: &mut self.result.traces,
                    line_coverage: &mut self.result.line_coverage,
                    deprecated_cheatcodes: &mut self.result.deprecated_cheatcodes,
                    inputs: &txes,
                    generate_stack_trace: true,
                    contract_decoder: Some(&*self.cr.contract_decoder),
                    revert_decoder: self.cr.revert_decoder,
                    fail_on_revert: self.cr.invariant_config.fail_on_revert,
                })
                .map_or(None, |result| result.stack_trace_result);
                self.result.invariant_replay_fail(
                    replayed_entirely,
                    &invariant_contract.invariant_function.name,
                    call_sequence,
                    stack_trace_result,
                    start.elapsed(),
                );
                return self.result;
            }
        }

        let invariant_result = match evm.invariant_fuzz(
            invariant_contract.clone(),
            &self.setup.fuzz_fixtures,
            &self.setup.deployed_libs,
        ) {
            Ok(x) => x,
            Err(e) => {
                let elapsed = start.elapsed();

                let stack_trace_result: StackTraceResult<HaltReasonT> =
                    if self.executor.tracer_records_steps() {
                        get_stack_trace(&*self.cr.contract_decoder, &self.result.traces)
                            .transpose()
                            .expect("traces are not empty")
                            .into()
                    } else if let Some(indeterminism_reasons) = e.indetereminism_reasons() {
                        indeterminism_reasons.into()
                    } else {
                        self.re_run_test_for_stack_traces(func, &[], self.setup.has_setup_method)
                            .into()
                    };
                self.result.stack_trace_result = Some(stack_trace_result);

                self.result.invariant_setup_fail(e, elapsed);

                return self.result;
            }
        };
        // Merge coverage collected during invariant run with test setup coverage.
        self.result.merge_coverages(invariant_result.line_coverage);

        let mut counterexample = None;
        let success = invariant_result.error.is_none();
        let reason = invariant_result
            .error
            .as_ref()
            .and_then(foundry_evm::executors::invariant::InvariantFuzzError::revert_reason);

        match invariant_result.error {
            // If invariants were broken, replay the error to collect logs and traces
            Some(error) => match error {
                InvariantFuzzError::BrokenInvariant(case_data)
                | InvariantFuzzError::Revert(case_data) => {
                    // Replay error to create counterexample and to collect logs, traces and
                    // coverage.
                    match replay_error(ReplayErrorArgs {
                        executor: self.clone_executor(),
                        failed_case: &case_data,
                        invariant_contract: &invariant_contract,
                        known_contracts: self.cr.known_contracts,
                        ided_contracts: identified_contracts.clone(),
                        logs: &mut self.result.logs,
                        traces: &mut self.result.traces,
                        coverage: &mut None,
                        deprecated_cheatcodes: &mut self.result.deprecated_cheatcodes,
                        generate_stack_trace: true,
                        contract_decoder: Some(&*self.cr.contract_decoder),
                        revert_decoder: self.cr.revert_decoder,
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
                                self.result.stack_trace_result = stack_trace_result;
                                self.result.reason = revert_reason;
                            }
                        }
                        Err(err) => {
                            error!(%err, "Failed to replay invariant error");
                        }
                    };
                }
                InvariantFuzzError::Abi(_)
                | InvariantFuzzError::Other(_)
                | InvariantFuzzError::MaxAssumeRejects(_) => {}
            },

            // If invariants ran successfully, replay the last run to collect logs and
            // traces.
            _ => {
                if let Err(err) = replay_run(ReplayRunArgs {
                    executor: self.clone_executor(),
                    invariant_contract: &invariant_contract,
                    known_contracts: self.cr.known_contracts,
                    ided_contracts: identified_contracts.clone(),
                    logs: &mut self.result.logs,
                    traces: &mut self.result.traces,
                    line_coverage: &mut self.result.line_coverage,
                    deprecated_cheatcodes: &mut self.result.deprecated_cheatcodes,
                    inputs: &invariant_result.last_run_inputs,
                    generate_stack_trace: false,
                    contract_decoder: Some(&*self.cr.contract_decoder),
                    revert_decoder: self.cr.revert_decoder,
                    fail_on_revert: self.cr.invariant_config.fail_on_revert,
                }) {
                    error!(%err, "Failed to replay last invariant run");
                }
            }
        }

        self.result.invariant_result(
            invariant_result.gas_report_traces,
            success,
            reason,
            counterexample,
            invariant_result.cases,
            invariant_result.reverts,
            invariant_result.metrics,
            invariant_result.failed_corpus_replays,
            start.elapsed(),
        );
        self.result
    }

    /// Runs a fuzzed test.
    ///
    /// Applies the before test txes (if any), fuzzes the current function and
    /// returns the `TestResult`.
    ///
    /// Before test txes are applied in order and state modifications committed
    /// to the EVM database (therefore the fuzz test will use the modified
    /// state). State modifications of before test txes and fuzz test are
    /// discarded after test ends, similar to `eth_call`.
    fn run_fuzz_test(mut self, func: &Function) -> TestResult<HaltReasonT> {
        let start = Instant::now();

        // Prepare fuzz test execution.
        if self.prepare_test(func, start).is_err() {
            return self.result;
        }

        let runner = self.fuzz_runner();
        let fuzz_config = self.cr.fuzz_config.clone();

        // Run fuzz test.
        let fuzzed_executor = FuzzedExecutor::new(
            self.executor.into_owned(),
            runner,
            self.cr.sender,
            fuzz_config,
        );
        let result = fuzzed_executor.fuzz(
            func,
            &self.setup.fuzz_fixtures,
            &self.setup.deployed_libs,
            self.setup.address,
            self.cr.revert_decoder,
        );
        self.result.fuzz_result(result, start.elapsed());

        self.result.stack_trace_result = if let Some(CounterExample::Single(counter_example)) =
            self.result.counterexample.as_ref()
        {
            let stack_trace_result: StackTraceResult<_> = if fuzzed_executor.tracer_records_steps()
            {
                get_stack_trace(&*self.cr.contract_decoder, &self.result.traces)
                    .transpose()
                    .expect("traces are not empty")
                    .into()
            } else if let Some(indeterminism_reasons) =
                counter_example.indeterminism_reasons.clone()
            {
                indeterminism_reasons.into()
            } else {
                re_run_fuzz_counterexample_for_stack_traces(
                    self.cr,
                    self.setup.address,
                    counter_example,
                    self.setup.has_setup_method,
                )
                .into()
            };
            Some(stack_trace_result)
        } else {
            None
        };

        self.result
    }

    /// Prepares single unit test and fuzz test execution:
    /// - set up the test result and executor
    /// - check if before test txes are configured and apply them in order
    ///
    /// Before test txes are arrays of arbitrary calldata obtained by calling
    /// the `beforeTest` function with test selector as a parameter.
    ///
    /// Unit tests within same contract (or even current test) are valid options
    /// for before test tx configuration. Test execution stops if any of
    /// before test txes fails.
    fn prepare_test(&mut self, func: &Function, start: Instant) -> Result<(), ()> {
        let address = self.setup.address;

        // Apply before test configured functions (if any).
        if self
            .cr
            .contract
            .abi
            .functions()
            .filter(|func| func.name.is_before_test_setup())
            .count()
            == 1
        {
            for calldata in self.executor.call_sol_default(
                address,
                &ITest::beforeTestSetupCall {
                    testSelector: func.selector(),
                },
            ) {
                // Apply before test configured calldata.
                if let Ok(call_result) = self.executor.to_mut().transact_raw(
                    self.cr.sender,
                    address,
                    calldata,
                    U256::ZERO,
                ) {
                    let reverted = call_result.reverted;

                    // Merge tx result traces in unit test result.
                    self.result.extend(call_result);

                    // To continue unit test execution the call should not revert.
                    if reverted {
                        self.result.single_fail(None, start.elapsed());
                        return Err(());
                    }
                } else {
                    self.result.single_fail(None, start.elapsed());
                    return Err(());
                }
            }
        }
        Ok(())
    }

    fn fuzz_runner(&self) -> TestRunner {
        let config = self.cr.fuzz_config;
        fuzzer_with_cases(
            config.seed,
            config.runs,
            config.max_test_rejects,
            config.file_failure_persistence(),
        )
    }

    fn invariant_runner(&self) -> TestRunner {
        let config = self.cr.invariant_config;
        fuzzer_with_cases(
            self.cr.fuzz_config.seed,
            config.runs,
            config.max_assume_rejects,
            None,
        )
    }

    fn clone_executor(
        &self,
    ) -> Executor<BlockT, TxT, EvmBuilderT, HaltReasonT, HardforkT, TransactionErrorT, ChainContextT>
    {
        self.executor.clone().into_owned()
    }

    /// Re-run the deployment, setup and test execution with expensive EVM step
    /// tracing to generate a stack trace for a failed test.
    fn re_run_test_for_stack_traces(
        &self,
        func: &Function,
        args: &[DynSolValue],
        needs_setup: bool,
    ) -> Result<Vec<StackTraceEntry>, StackTraceError<HaltReasonT>> {
        let mut executor = self.cr.executor_builder.clone().build()?;

        // We only need light-weight tracing for setup to be able to match contract
        // codes to contact addresses.
        executor.inspector_mut().tracing(TracingMode::WithoutSteps);
        let setup = self.cr.setup(&mut executor, needs_setup);
        if let Some(reason) = setup.reason {
            // If this function was called, the setup succeeded during test execution, so
            // this is an unexpected failure.
            return Err(StackTraceError::FailingSetup(reason));
        }

        // Collect EVM step traces that are needed for stack trace generation.
        executor.inspector_mut().tracing(TracingMode::WithSteps);

        // Run unit test
        let new_traces = match executor.call(
            self.cr.sender,
            setup.address,
            func,
            args,
            U256::ZERO,
            Some(self.cr.revert_decoder),
        ) {
            Ok(res) => res.raw.traces,
            Err(EvmError::Execution(err)) => err.raw.traces,
            Err(err) => return Err(err.into()),
        }
        .expect("enabled tracing");

        let mut traces = setup.traces;
        traces.push((TraceKind::Execution, new_traces));

        get_stack_trace(&*self.cr.contract_decoder, &traces)
            .transpose()
            .expect("traces are not empty")
    }
}

/// Re-run the deployment, setup and test execution with expensive EVM step
/// tracing to generate a stack trace for a fuzz counterexample.
/// This is a standalone function to allow partially moving in the parent.
fn re_run_fuzz_counterexample_for_stack_traces<
    BlockT: BlockEnvTr,
    ChainContextT: 'static + ChainContextTr,
    EvmBuilderT: 'static
        + EvmBuilderTrait<BlockT, ChainContextT, HaltReasonT, HardforkT, TransactionErrorT, TxT>,
    HaltReasonT: 'static + HaltReasonTrait + TryInto<EvmHaltReason>,
    HardforkT: HardforkTr,
    TransactionErrorT: TransactionErrorTrait,
    TxT: TransactionEnvTr,
    NestedTraceDecoderT: SyncNestedTraceDecoder<HaltReasonT>,
>(
    contract_runner: &ContractRunner<
        '_,
        BlockT,
        ChainContextT,
        EvmBuilderT,
        HaltReasonT,
        HardforkT,
        TransactionErrorT,
        TxT,
        NestedTraceDecoderT,
    >,
    address: Address,
    counter_example: &BaseCounterExample,
    needs_setup: bool,
) -> Result<Vec<StackTraceEntry>, StackTraceError<HaltReasonT>> {
    let mut executor = contract_runner.executor_builder.clone().build()?;

    // We only need light-weight tracing for setup to be able to match contract
    // codes to contact addresses.
    executor.inspector_mut().tracing(TracingMode::WithoutSteps);
    let setup = contract_runner.setup(&mut executor, needs_setup);
    if let Some(reason) = setup.reason {
        // If this function was called, the setup succeeded during test execution, so
        // this is an unexpected failure.
        return Err(StackTraceError::FailingSetup(reason));
    }

    // Collect EVM step traces that are needed for stack trace generation.
    executor.inspector_mut().tracing(TracingMode::WithSteps);

    // Run counterexample test
    let call = executor
        .call_raw(
            contract_runner.sender,
            address,
            counter_example.calldata.clone(),
            U256::ZERO,
        )
        .map_err(|err| StackTraceError::Evm(err.to_string()))?;

    let mut traces = setup.traces;
    traces.push((TraceKind::Execution, call.traces.expect("tracing is on")));

    get_stack_trace(&*contract_runner.contract_decoder, &traces)
        .transpose()
        .expect("traces are not empty")
}

fn fuzzer_with_cases(
    seed: Option<U256>,
    cases: u32,
    max_global_rejects: u32,
    file_failure_persistence: Option<Box<dyn FailurePersistence>>,
) -> TestRunner {
    let config = proptest::test_runner::Config {
        failure_persistence: file_failure_persistence,
        cases,
        max_global_rejects,
        // Disable proptest shrink: for fuzz tests we provide single counterexample,
        // for invariant tests we shrink outside proptest.
        max_shrink_iters: 0,
        ..Default::default()
    };

    if let Some(seed) = seed {
        trace!(target: "forge::test", %seed, "building deterministic fuzzer");
        let rng = TestRng::from_seed(RngAlgorithm::ChaCha, &seed.to_be_bytes::<32>());
        TestRunner::new_with_rng(config, rng)
    } else {
        trace!(target: "forge::test", "building stochastic fuzzer");
        TestRunner::new(config)
    }
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

/// Returns `true` if the function is a test function that matches the given
/// filter.
fn is_matching_test(func: &Function, filter: &dyn TestFilter) -> bool {
    func.is_any_test() && filter.matches_test(&func.signature())
}
