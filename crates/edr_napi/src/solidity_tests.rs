mod runner;
mod test_results;
mod test_suite;

use std::path::Path;

use forge::TestFilter;
use napi::{
    threadsafe_function::{
        ErrorStrategy, ThreadSafeCallContext, ThreadsafeFunction, ThreadsafeFunctionCallMode,
    },
    tokio,
    tokio::runtime,
    Env, JsFunction,
};
use napi_derive::napi;

use crate::solidity_tests::{
    runner::build_runner, test_results::SuiteResult, test_suite::TestSuite,
};

/// Executes solidity tests.
#[napi]
pub struct SolidityTestRunner {
    /// The callback to call with the results as they become available.
    results_callback_fn: ThreadsafeFunction<SuiteResult>,
}

// The callback has to be passed in the constructor because it's not `Send`.
#[napi]
impl SolidityTestRunner {
    /// Creates a new instance of the SolidityTestRunner.
    #[doc = "Creates a new instance of the SolidityTestRunner. The callback function will be called with suite results as they finish."]
    #[napi(constructor)]
    pub fn new(env: Env, results_callback: JsFunction) -> napi::Result<Self> {
        let mut results_callback_fn: ThreadsafeFunction<_, ErrorStrategy::CalleeHandled> =
            results_callback.create_threadsafe_function(
                // Unbounded queue size
                0,
                |ctx: ThreadSafeCallContext<SuiteResult>| Ok(vec![ctx.value]),
            )?;

        // Allow the event loop to exit before the function is destroyed
        results_callback_fn.unref(&env)?;

        Ok(Self {
            results_callback_fn,
        })
    }

    #[doc = "Runs the given test suites."]
    #[napi]
    pub async fn run_tests(&self, test_suites: Vec<TestSuite>) -> napi::Result<Vec<SuiteResult>> {
        let test_suites = test_suites
            .into_iter()
            .map(|item| Ok((item.id.try_into()?, item.contract.try_into()?)))
            .collect::<Result<Vec<_>, napi::Error>>()?;
        let mut runner = build_runner(test_suites)?;

        let (tx_results, mut rx_results) =
            tokio::sync::mpsc::unbounded_channel::<(String, forge::result::SuiteResult)>();
        let (tx_end_result, mut rx_end_result) = tokio::sync::mpsc::unbounded_channel();

        let callback_fn = self.results_callback_fn.clone();
        let runtime = runtime::Handle::current();
        runtime.spawn(async move {
            let mut results = Vec::<(String, forge::result::SuiteResult)>::new();

            while let Some(name_and_suite_result) = rx_results.recv().await {
                results.push(name_and_suite_result.clone());
                // Blocking mode won't block in our case because the function was created with
                // unlimited queue size https://github.com/nodejs/node-addon-api/blob/main/doc/threadsafe_function.md#blockingcall--nonblockingcall
                callback_fn.call(
                    Ok(name_and_suite_result.into()),
                    ThreadsafeFunctionCallMode::Blocking,
                );
            }

            let js_suite_results = results
                .into_iter()
                .map(Into::into)
                .collect::<Vec<SuiteResult>>();
            tx_end_result
                .send(js_suite_results)
                .expect("Failed to send test result");
        });

        runner.test_async_channel(&EverythingFilter, tx_results);

        let results = rx_end_result.recv().await.ok_or_else(|| {
            napi::Error::new(napi::Status::GenericFailure, "Failed to receive end result")
        })?;

        Ok(results)
    }
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
