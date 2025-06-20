//! Test config.

use std::{collections::BTreeMap, marker::PhantomData};

use derive_where::derive_where;
use edr_eth::{l1, spec::HaltReasonTrait};
use edr_solidity::{
    contract_decoder::{ContractDecoderError, NestedTraceDecoder},
    nested_trace::NestedTrace,
};
use edr_solidity_tests::{
    result::{SuiteResult, TestStatus},
    MultiContractRunner,
};
use foundry_evm::{
    decode::decode_console_logs,
    evm_context::{
        BlockEnvTr, ChainContextTr, EvmBuilderTrait, HardforkTr, TransactionEnvTr,
        TransactionErrorTrait,
    },
    traces::{decode_trace_arena, render_trace_arena, CallTraceDecoderBuilder},
};
use futures::future::join_all;
use itertools::Itertools;

use crate::helpers::{tracing::init_tracing_for_solidity_tests, SolidityTestFilter};

/// How to execute a test run.
pub struct TestConfig<
    BlockT: BlockEnvTr,
    ChainContextT: 'static + ChainContextTr + Send + Sync,
    EvmBuilderT: 'static
        + EvmBuilderTrait<
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
> {
    pub runner: MultiContractRunner<
        BlockT,
        ChainContextT,
        EvmBuilderT,
        HaltReasonT,
        HardforkT,
        NoOpContractDecoder<HaltReasonT>,
        TransactionErrorT,
        TransactionT,
    >,
    pub should_fail: bool,
    pub filter: SolidityTestFilter,
}

impl<
        BlockT: BlockEnvTr,
        ChainContextT: 'static + ChainContextTr + Send + Sync,
        EvmBuilderT: 'static
            + EvmBuilderTrait<
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
    TestConfig<
        BlockT,
        ChainContextT,
        EvmBuilderT,
        HaltReasonT,
        HardforkT,
        TransactionErrorT,
        TransactionT,
    >
{
    pub fn new(
        runner: MultiContractRunner<
            BlockT,
            ChainContextT,
            EvmBuilderT,
            HaltReasonT,
            HardforkT,
            NoOpContractDecoder<HaltReasonT>,
            TransactionErrorT,
            TransactionT,
        >,
    ) -> Self {
        Self::with_filter(runner, SolidityTestFilter::matches_all())
    }

    pub fn with_filter(
        runner: MultiContractRunner<
            BlockT,
            ChainContextT,
            EvmBuilderT,
            HaltReasonT,
            HardforkT,
            NoOpContractDecoder<HaltReasonT>,
            TransactionErrorT,
            TransactionT,
        >,
        filter: SolidityTestFilter,
    ) -> Self {
        init_tracing_for_solidity_tests();
        Self {
            runner,
            should_fail: false,
            filter,
        }
    }

    pub fn should_fail(self) -> Self {
        self.set_should_fail(true)
    }

    pub fn set_should_fail(mut self, should_fail: bool) -> Self {
        self.should_fail = should_fail;
        self
    }

    /// Executes the test runner
    pub async fn test(self) -> BTreeMap<String, SuiteResult<HaltReasonT>> {
        self.runner.test_collect(self.filter).await
    }

    pub async fn run(self) {
        self.try_run().await.unwrap();
    }

    /// Executes the test case
    ///
    /// Returns an error if
    ///    * filter matched 0 test cases
    ///    * a test results deviates from the configured `should_fail` setting
    pub async fn try_run(self) -> eyre::Result<()> {
        let should_fail = self.should_fail;
        let known_contracts = self.runner.known_contracts().clone();
        let suite_result = self.test().await;
        if suite_result.is_empty() {
            eyre::bail!("empty test result");
        }
        for (_, SuiteResult { test_results, .. }) in suite_result {
            for (test_name, mut result) in test_results {
                if should_fail && (result.status == TestStatus::Success)
                    || !should_fail && (result.status == TestStatus::Failure)
                {
                    let logs = decode_console_logs(&result.logs);
                    let outcome = if should_fail { "fail" } else { "pass" };
                    let call_trace_decoder = CallTraceDecoderBuilder::default()
                        .with_known_contracts(&known_contracts)
                        .build();
                    let decoded_traces = join_all(result.traces.iter_mut().map(|(_, arena)| {
                        let decoder = &call_trace_decoder;
                        async move {
                            decode_trace_arena(arena, decoder).await;
                            render_trace_arena(arena)
                        }
                    }))
                    .await
                    .into_iter()
                    .collect::<Vec<String>>();
                    eyre::bail!(
                        "Test {} did not {} as expected.\nReason: {:?}\nLogs:\n{}\n\nTraces:\n{}",
                        test_name,
                        outcome,
                        result.reason,
                        logs.join("\n"),
                        decoded_traces.into_iter().format("\n"),
                    )
                }
            }
        }

        Ok(())
    }
}

#[derive_where(Clone, Debug, Default)]
pub struct NoOpContractDecoder<HaltReasonT: HaltReasonTrait> {
    _phantom: PhantomData<HaltReasonT>,
}

impl<HaltReasonT: HaltReasonTrait> NestedTraceDecoder<HaltReasonT>
    for NoOpContractDecoder<HaltReasonT>
{
    fn try_to_decode_nested_trace(
        &self,
        nested_trace: NestedTrace<HaltReasonT>,
    ) -> Result<NestedTrace<HaltReasonT>, ContractDecoderError> {
        Ok(nested_trace)
    }
}

/// A helper to assert the outcome of multiple tests with helpful assert
/// messages
#[track_caller]
#[allow(clippy::type_complexity)]
pub fn assert_multiple<HaltReasonT: HaltReasonTrait>(
    actuals: &BTreeMap<String, SuiteResult<HaltReasonT>>,
    expecteds: BTreeMap<
        &str,
        Vec<(
            &str,
            bool,
            Option<String>,
            Option<Vec<String>>,
            Option<usize>,
        )>,
    >,
) {
    assert_eq!(
        actuals.len(),
        expecteds.len(),
        "We did not run as many contracts as we expected"
    );
    for (contract_name, tests) in &expecteds {
        assert!(
            actuals.contains_key(*contract_name),
            "We did not run the contract {contract_name}"
        );

        assert_eq!(
            actuals[*contract_name].len(),
            expecteds[contract_name].len(),
            "We did not run as many test functions as we expected for {contract_name}"
        );
        for (test_name, should_pass, reason, expected_logs, expected_warning_count) in tests {
            let logs = &actuals[*contract_name].test_results[*test_name].decoded_logs;

            let warnings_count = &actuals[*contract_name].warnings.len();

            if *should_pass {
                assert!(
                    actuals[*contract_name].test_results[*test_name].status == TestStatus::Success,
                    "Test {} did not pass as expected.\nReason: {:?}\nLogs:\n{}",
                    test_name,
                    actuals[*contract_name].test_results[*test_name].reason,
                    logs.join("\n")
                );
            } else {
                assert!(
                    actuals[*contract_name].test_results[*test_name].status == TestStatus::Failure,
                    "Test {} did not fail as expected.\nLogs:\n{}",
                    test_name,
                    logs.join("\n")
                );
                assert_eq!(
                    actuals[*contract_name].test_results[*test_name].reason, *reason,
                    "Failure reason for test {test_name} did not match what we expected."
                );
            }

            if let Some(expected_logs) = expected_logs {
                assert_eq!(
                    logs,
                    expected_logs,
                    "Logs did not match for test {}.\nExpected:\n{}\n\nGot:\n{}",
                    test_name,
                    expected_logs.join("\n"),
                    logs.join("\n")
                );
            }

            if let Some(expected_warning_count) = expected_warning_count {
                assert_eq!(
                    warnings_count, expected_warning_count,
                    "Test {test_name} did not pass as expected. Expected:\n{expected_warning_count}Got:\n{warnings_count}"
                );
            }
        }
    }
}
