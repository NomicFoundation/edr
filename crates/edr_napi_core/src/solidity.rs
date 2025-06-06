pub mod config;
mod factory;

use std::sync::Arc;

use edr_eth::l1;
use edr_solidity::contract_decoder::SyncNestedTraceDecoder;
use edr_solidity_tests::{
    evm_context::{BlockEnvTr, ChainContextTr, EvmBuilderTrait, HardforkTr, TransactionEnvTr},
    multi_runner::OnTestSuiteCompletedFn,
    MultiContractRunner, TestFilterConfig,
};

pub use self::factory::SyncTestRunnerFactory;

pub trait SyncTestRunner: Send + Sync {
    fn run_tests(
        self: Box<Self>,
        test_filter: Arc<TestFilterConfig>,
        // TODO: Convert `l1::HaltReason` to `serde_json::Value`
        on_test_suite_completed_fn: Arc<dyn OnTestSuiteCompletedFn<l1::HaltReason>>,
    ) -> napi::Result<()>;
}

impl<
        BlockT: BlockEnvTr,
        ChainContextT: 'static + ChainContextTr + Send + Sync,
        EvmBuilderT: 'static + EvmBuilderTrait<BlockT, ChainContextT, l1::HaltReason, HardforkT, TransactionT>,
        // HaltReasonT: 'static + HaltReasonTrait + Into<InstructionResult> + Send + Sync +
        // serde::Serialize,
        HardforkT: HardforkTr,
        NestedTraceDecoderT: SyncNestedTraceDecoder<l1::HaltReason>,
        TransactionT: TransactionEnvTr,
    > SyncTestRunner
    for MultiContractRunner<
        BlockT,
        ChainContextT,
        EvmBuilderT,
        l1::HaltReason,
        HardforkT,
        NestedTraceDecoderT,
        TransactionT,
    >
{
    fn run_tests(
        self: Box<Self>,
        test_filter: Arc<TestFilterConfig>,
        on_test_suite_completed_fn: Arc<dyn OnTestSuiteCompletedFn<l1::HaltReason>>,
    ) -> napi::Result<()> {
        self.test(test_filter, on_test_suite_completed_fn);

        Ok(())
    }
}
