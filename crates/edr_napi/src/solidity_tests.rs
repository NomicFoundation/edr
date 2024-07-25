mod config;
mod runner;
mod test_results;
mod test_suite;

use std::{path::Path, sync::Arc};

use forge::TestFilter;
use napi::{
    threadsafe_function::{
        ErrorStrategy, ThreadSafeCallContext, ThreadsafeFunction, ThreadsafeFunctionCallMode,
    },
    tokio,
    tokio::runtime,
    JsFunction,
};
use napi_derive::napi;

use crate::solidity_tests::{
    runner::build_runner, test_results::SuiteResult, test_suite::TestSuite,
};

/// Executes Solidity tests.
///
/// The function will return as soon as test execution is started.
/// The progress callback will be called with the results of each test suite.
/// It is up to the caller to track how many times the callback is called to
/// know when all tests are done.
// False positive from Clippy. The function is exposed through the FFI.
#[allow(dead_code)]
#[napi(
    ts_args_type = "test_suites: Array<TestSuite>, gas_report: boolean, progress_callback: (result: SuiteResult) => void"
)]
pub fn run_solidity_tests(
    test_suites: Vec<TestSuite>,
    gas_report: bool,
    progress_callback: JsFunction,
) -> napi::Result<()> {
    let results_callback_fn: ThreadsafeFunction<_, ErrorStrategy::Fatal> = progress_callback
        .create_threadsafe_function(
            // Unbounded queue size
            0,
            |ctx: ThreadSafeCallContext<SuiteResult>| Ok(vec![ctx.value]),
        )?;

    let test_suites = test_suites
        .into_iter()
        .map(|item| Ok((item.id.try_into()?, item.contract.try_into()?)))
        .collect::<Result<Vec<_>, napi::Error>>()?;
    let runner = build_runner(test_suites, gas_report)?;

    let (tx_results, mut rx_results) =
        tokio::sync::mpsc::unbounded_channel::<(String, forge::result::SuiteResult)>();

    let runtime = runtime::Handle::current();
    runtime.spawn(async move {
        while let Some(name_and_suite_result) = rx_results.recv().await {
            let callback_arg = name_and_suite_result.into();
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

    // Returns immediately after test suite execution is started
    runner.test_hardhat(Arc::new(EverythingFilter), tx_results);

    Ok(())
}

struct EverythingFilter;

impl TestFilter for EverythingFilter {
    fn matches_test(&self, _test_name: &str) -> bool {
        true
    }

    fn matches_contract(&self, _contract_name: &str) -> bool {
        true
    }

    fn matches_path(&self, _path: &Path) -> bool {
        true
    }
}
