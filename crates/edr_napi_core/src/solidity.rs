pub mod config;
mod factory;

use std::sync::Arc;

use edr_evm_spec::{EvmHaltReason, HaltReasonTrait};
use edr_solidity::contract_decoder::SyncNestedTraceDecoder;
use edr_solidity_tests::{
    evm_context::{
        BlockEnvTr, ChainContextTr, EvmBuilderTrait, HardforkTr, TransactionEnvTr,
        TransactionErrorTrait,
    },
    multi_runner::{OnTestSuiteCompletedFn, SolidityTestResult, SuiteResultAndArtifactId},
    MultiContractRunner, TestFilterConfig,
};

pub use self::factory::SyncTestRunnerFactory;

pub trait SyncTestRunner: Send + Sync {
    fn run_tests(
        self: Box<Self>,
        runtime: tokio::runtime::Handle,
        test_filter: Arc<TestFilterConfig>,
        on_test_suite_completed_fn: Arc<dyn OnTestSuiteCompletedFn<String>>,
    ) -> napi::Result<SolidityTestResult>;
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
        HaltReasonT: 'static + HaltReasonTrait + TryInto<EvmHaltReason> + Send + Sync + serde::Serialize,
        HardforkT: HardforkTr,
        NestedTraceDecoderT: SyncNestedTraceDecoder<HaltReasonT>,
        TransactionErrorT: TransactionErrorTrait,
        TransactionT: TransactionEnvTr,
    > SyncTestRunner
    for MultiContractRunner<
        BlockT,
        ChainContextT,
        EvmBuilderT,
        HaltReasonT,
        HardforkT,
        NestedTraceDecoderT,
        TransactionErrorT,
        TransactionT,
    >
{
    fn run_tests(
        self: Box<Self>,
        runtime: tokio::runtime::Handle,
        test_filter: Arc<TestFilterConfig>,
        on_test_suite_completed_fn: Arc<dyn OnTestSuiteCompletedFn<String>>,
    ) -> napi::Result<SolidityTestResult> {
        let test_result = self
            .test(
                runtime,
                test_filter,
                Arc::new(
                    move |SuiteResultAndArtifactId {
                              artifact_id,
                              result,
                          }| {
                        let result = result.map_halt_reason(|halt_reason: HaltReasonT| {
                            serde_json::to_string(&halt_reason)
                                .expect("Failed to serialize halt reason")
                        });

                        on_test_suite_completed_fn(SuiteResultAndArtifactId {
                            artifact_id,
                            result,
                        });
                    },
                ),
            )
            .map_err(|err| napi::Error::from_reason(err.to_string()))?;

        Ok(test_result)
    }
}
