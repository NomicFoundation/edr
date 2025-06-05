mod artifact;
mod config;
mod runner;
mod test_results;

use std::sync::Arc;

use artifact::Artifact;
use edr_solidity_tests::TestFilterConfig;
use napi::{
    threadsafe_function::{
        ErrorStrategy, ThreadSafeCallContext, ThreadsafeFunction, ThreadsafeFunctionCallMode,
    },
    tokio,
    tokio::runtime,
    JsFunction,
};
use napi_derive::napi;

use crate::{
    provider::TracingConfigWithBuffers,
    solidity_tests::{
        artifact::ArtifactId, config::SolidityTestRunnerConfigArgs, runner::build_runner,
        test_results::SuiteResult,
    },
};

/// Executes Solidity tests.
///
/// The function will return as soon as test execution is started.
/// The progress callback will be called with the results of each test suite.
/// It is up to the caller to track how many times the callback is called to
/// know when all tests are done.
/// The error callback is called if an invalid configuration value is provided.
// False positive from Clippy. The function is exposed through the FFI.
#[allow(dead_code)]
#[napi]
pub fn run_solidity_tests(
    artifacts: Vec<Artifact>,
    test_suites: Vec<ArtifactId>,
    config_args: SolidityTestRunnerConfigArgs,
    tracing_config: TracingConfigWithBuffers,
    #[napi(ts_arg_type = "(result: SuiteResult) => void")] progress_callback: JsFunction,
    #[napi(ts_arg_type = "(error: Error) => void")] error_callback: JsFunction,
) -> napi::Result<()> {
    let results_callback_fn: ThreadsafeFunction<_, ErrorStrategy::Fatal> = progress_callback
        .create_threadsafe_function(
            // Unbounded queue size
            0,
            |ctx: ThreadSafeCallContext<SuiteResult>| Ok(vec![ctx.value]),
        )?;
    let error_callback_fn: ThreadsafeFunction<_, ErrorStrategy::Fatal> = error_callback
        .create_threadsafe_function(
            // Unbounded queue size
            0,
            |ctx: ThreadSafeCallContext<napi::Error>| Ok(vec![ctx.value]),
        )?;

    let (tx_results, mut rx_results) = tokio::sync::mpsc::unbounded_channel::<(
        edr_solidity::artifacts::ArtifactId,
        edr_solidity_tests::result::SuiteResult<edr_eth::l1::HaltReason>,
    )>();

    let test_filter: Arc<TestFilterConfig> = Arc::new(config_args.try_get_test_filter()?);
    let include_traces = config_args.include_traces.unwrap_or_default();

    let runtime = runtime::Handle::current();
    runtime.spawn(async move {
        let runner = match build_runner(artifacts, test_suites, config_args, tracing_config).await {
            Ok(runner) => runner,
            Err(error) => {
                let call_status =
                    error_callback_fn.call(error, ThreadsafeFunctionCallMode::Blocking);
                // This should always succeed since we're using an unbounded queue. We add an
                // assertion for completeness.
                assert!(
                    matches!(call_status, napi::Status::Ok),
                    "Failed to call callback with status {call_status:?}"
                );
                return;
            }
        };

        // Returns immediately after test suite execution is started
        runner.test(test_filter, tx_results);

        while let Some((id, suite_result)) = rx_results.recv().await {
            let callback_arg = SuiteResult::new(id, suite_result, include_traces);
            // Blocking mode won't block in our case because the function was created with
            // unlimited queue size https://github.com/nodejs/node-addon-api/blob/main/doc/threadsafe_function.md#blockingcall--nonblockingcall
            let call_status =
                results_callback_fn.call(callback_arg, ThreadsafeFunctionCallMode::Blocking);
            // This should always succeed since we're using an unbounded queue. We add an
            // assertion for completeness.
            assert!(
                matches!(call_status, napi::Status::Ok),
                "Failed to call callback with status {call_status:?}"
            );
        }
    });

    Ok(())
}
