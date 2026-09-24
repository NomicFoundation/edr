//! Fuzz tests.

use std::collections::BTreeMap;

use alloy_primitives::{Bytes, U256};
use edr_gas_report::GasReportExecutionStatus;
use edr_solidity_tests::{
    fuzz::{BaseCounterExample, CounterExample},
    inline_config::InlineConfigProfiles,
    result::{SuiteResult, TestKind, TestStatus},
};

use crate::helpers::{assert_multiple, SolidityTestFilter, TEST_DATA_DEFAULT};

#[tokio::test(flavor = "multi_thread")]
async fn test_fuzz() {
    let filter = SolidityTestFilter::new(".*", ".*", ".*fuzz/")
        .exclude_tests(
            &[
                r"invariantCounter",
                r"testIncrement\(address\)",
                r"testNeedle\(uint256\)",
                r"testSuccessChecker\(uint256\)",
                r"testSuccessChecker2\(int256\)",
                r"testSuccessChecker3\(uint32\)",
                r"testFuzz_SetNumberAssert\(uint256\)",
                r"testFuzz_SetNumberRequire\(uint256\)",
                r"test_fuzz_bound\(uint256\)",
                r"testImmutableOwner\(address\)",
                r"testStorageOwner\(address\)",
                r"testFuzzWithRejects\(uint256\)",
            ]
            .join("|"),
        )
        .exclude_paths("invariant")
        .exclude_contracts("FuzzConfigOverrideTest|FuzzProfileOverrideTest");
    let runner = TEST_DATA_DEFAULT.runner().await;
    let suite_result = runner.test_collect(filter).await.suite_results;

    assert!(!suite_result.is_empty());

    for (_, SuiteResult { test_results, .. }) in suite_result {
        for (test_name, result) in test_results {
            match test_name.as_str() {
                "testPositive(uint256)"
                | "testPositive(int256)"
                | "testSuccessfulFuzz(uint128,uint128)"
                | "testArray(uint64[2])"
                | "testArrayPreBytecodeHash(uint64[2])"
                | "testToStringFuzz(bytes32)" => assert_eq!(
                    result.status,
                    TestStatus::Success,
                    "Test {} did not pass as expected.\nReason: {:?}\nLogs:\n{}",
                    test_name,
                    result.reason,
                    result.decoded_logs.join("\n")
                ),
                _ => assert_eq!(
                    result.status,
                    TestStatus::Failure,
                    "Test {} did not fail as expected.\nReason: {:?}\nLogs:\n{}",
                    test_name,
                    result.reason,
                    result.decoded_logs.join("\n")
                ),
            }
        }
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn test_successful_fuzz_cases() {
    let filter = SolidityTestFilter::new(".*", ".*", ".*fuzz/FuzzPositive")
        .exclude_tests(r"invariantCounter|testIncrement\(address\)|testNeedle\(uint256\)")
        .exclude_paths("invariant");
    let runner = TEST_DATA_DEFAULT.runner().await;
    let suite_result = runner.test_collect(filter).await.suite_results;

    assert!(!suite_result.is_empty());

    for (_, SuiteResult { test_results, .. }) in suite_result {
        for (test_name, result) in test_results {
            match test_name.as_str() {
                "testSuccessChecker(uint256)"
                | "testSuccessChecker2(int256)"
                | "testSuccessChecker3(uint32)" => assert_eq!(
                    result.status,
                    TestStatus::Success,
                    "Test {} did not pass as expected.\nReason: {:?}\nLogs:\n{}",
                    test_name,
                    result.reason,
                    result.decoded_logs.join("\n")
                ),
                _ => {}
            }
        }
    }
}

/// Test that showcases PUSH collection on normal fuzzing. Ignored until we
/// collect them in a smarter way.
/// Disabled in <https://github.com/foundry-rs/foundry/pull/2724>
#[tokio::test(flavor = "multi_thread")]
#[ignore]
async fn test_fuzz_collection() {
    let filter = SolidityTestFilter::new(".*", ".*", ".*fuzz/FuzzCollection.t.sol");
    let mut config = TEST_DATA_DEFAULT.config_with_mock_rpc();
    config.invariant.depth = 100;
    config.invariant.runs = 1000;
    config.fuzz.runs = 1000;
    config.fuzz.seed = Some(U256::from(6u32));
    let runner = TEST_DATA_DEFAULT.runner_with_fuzz_persistence(config).await;
    let results = runner.test_collect(filter).await.suite_results;

    assert_multiple(
        &results,
        BTreeMap::from([(
            "default/fuzz/FuzzCollection.t.sol:SampleContractTest",
            vec![
                (
                    "invariantCounter",
                    false,
                    Some("broken counter.".into()),
                    None,
                    None,
                ),
                (
                    "testIncrement(address)",
                    false,
                    Some("Call did not revert as expected".into()),
                    None,
                    None,
                ),
                (
                    "testNeedle(uint256)",
                    false,
                    Some("needle found.".into()),
                    None,
                    None,
                ),
            ],
        )]),
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn test_persist_fuzz_failure() {
    let filter = SolidityTestFilter::new(".*", ".*", ".*fuzz/FuzzFailurePersist.t.sol");
    let persist_dir = tempfile::tempdir().unwrap();
    let mut config = TEST_DATA_DEFAULT.config_with_mock_rpc();
    config.fuzz.runs = 1000;
    config.fuzz.seed = None;
    config.fuzz.failure_persist_dir = Some(persist_dir.path().to_path_buf());
    config.fuzz.failure_persist_file = "testfailure".to_string();
    let runner = TEST_DATA_DEFAULT.runner_with_config(config.clone()).await;

    macro_rules! get_failure_result {
        ($runner:ident) => {
            $runner
                .clone()
                .test_collect(filter.clone()).await
                .suite_results
                .get("default/fuzz/FuzzFailurePersist.t.sol:FuzzFailurePersistTest")
                .unwrap()
                .test_results
                .get("test_persist_fuzzed_failure(uint256,int256,address,bool,string,(address,uint256),address[])")
                .unwrap()
                .counterexample
                .clone()
        };
    }

    // record initial counterexample calldata
    let initial_counterexample = get_failure_result!(runner);
    let initial_calldata = match initial_counterexample {
        Some(CounterExample::Single(counterexample)) => counterexample.calldata,
        _ => Bytes::new(),
    };

    // the counterexample is persisted as JSON at
    // `<failure_persist_dir>/<failure_persist_file>/<contract>/<test>`
    let failure_file = persist_dir
        .path()
        .join("testfailure")
        .join("FuzzFailurePersistTest")
        .join("test_persist_fuzzed_failure");
    let persisted: BaseCounterExample =
        serde_json::from_slice(&std::fs::read(&failure_file).unwrap()).unwrap();
    assert_eq!(persisted.calldata, initial_calldata);

    // run several times and compare counterexamples calldata
    for i in 0..10 {
        let new_calldata = match get_failure_result!(runner) {
            Some(CounterExample::Single(counterexample)) => counterexample.calldata,
            _ => Bytes::new(),
        };
        // calldata should be the same with the initial one
        assert_eq!(initial_calldata, new_calldata, "run {i}");
    }

    // write new failure in different file, but keep the same directory
    config.fuzz.failure_persist_file = "failure1".to_string();
    let runner = TEST_DATA_DEFAULT.runner_with_config(config).await;
    let new_calldata = match get_failure_result!(runner) {
        Some(CounterExample::Single(counterexample)) => counterexample.calldata,
        _ => Bytes::new(),
    };
    // no failure is persisted under the new name so new calldata is generated
    assert_ne!(initial_calldata, new_calldata);
    assert!(persist_dir
        .path()
        .join("failure1")
        .join("FuzzFailurePersistTest")
        .join("test_persist_fuzzed_failure")
        .is_file());
}

/// Older EDR versions persisted fuzz failures as a single `proptest` seed file
/// at `<failure_persist_dir>/<failure_persist_file>`. It must be replaced by
/// the failure directory instead of blocking persistence.
#[tokio::test(flavor = "multi_thread")]
async fn test_persist_fuzz_failure_replaces_legacy_seed_file() {
    let filter = SolidityTestFilter::new(".*", ".*", ".*fuzz/FuzzFailurePersist.t.sol");
    let persist_dir = tempfile::tempdir().unwrap();
    let legacy_file = persist_dir.path().join("testfailure");
    std::fs::write(
        &legacy_file,
        "# Seeds for failure cases proptest has generated in the past.\n\
         cc 0000000000000000000000000000000000000000000000000000000000000000\n",
    )
    .unwrap();

    let mut config = TEST_DATA_DEFAULT.config_with_mock_rpc();
    config.fuzz.failure_persist_dir = Some(persist_dir.path().to_path_buf());
    config.fuzz.failure_persist_file = "testfailure".to_string();
    let runner = TEST_DATA_DEFAULT.runner_with_config(config).await;
    let results = runner.test_collect(filter).await.suite_results;
    let result = results
        .get("default/fuzz/FuzzFailurePersist.t.sol:FuzzFailurePersistTest")
        .unwrap()
        .test_results
        .get("test_persist_fuzzed_failure(uint256,int256,address,bool,string,(address,uint256),address[])")
        .unwrap();
    assert!(matches!(
        result.counterexample,
        Some(CounterExample::Single(_))
    ));

    assert!(legacy_file.is_dir());
    assert!(legacy_file
        .join("FuzzFailurePersistTest")
        .join("test_persist_fuzzed_failure")
        .is_file());
}

#[tokio::test(flavor = "multi_thread")]
async fn test_fuzz_gas_report() {
    let filter = SolidityTestFilter::new(".*", ".*", ".*fuzz/FuzzCollection.t.sol");
    let mut config = TEST_DATA_DEFAULT.config_with_mock_rpc();
    config.invariant.depth = 100;
    config.invariant.runs = 1000;
    config.fuzz.runs = 1000;
    config.fuzz.seed = Some(U256::from(6u32));
    config.generate_gas_report = true;
    let runner = TEST_DATA_DEFAULT.runner_with_fuzz_persistence(config).await;
    let test_result = runner.test_collect(filter).await.test_result;

    assert!(test_result.gas_report.is_some());

    let gas_report = test_result.gas_report.as_ref().unwrap();
    let sample_contract_report = gas_report
        .contracts
        .get("default/fuzz/FuzzCollection.t.sol:SampleContract")
        .unwrap();

    assert_eq!(sample_contract_report.deployments.len(), 1);
    let deployment = sample_contract_report.deployments.first().unwrap();

    // Assert with 10% tolerance
    assert_close!(deployment.gas, 224_987, 0.1);
    assert_close!(deployment.size, 743, 0.1);
    assert_eq!(deployment.status, GasReportExecutionStatus::Success);

    assert_eq!(sample_contract_report.functions.len(), 6);
    assert!(sample_contract_report.functions.contains_key("counterX2()"));
    assert!(sample_contract_report
        .functions
        .contains_key("compare(uint256)"));
    assert!(sample_contract_report
        .functions
        .contains_key("breakTheInvariant(uint256)"));
    assert!(sample_contract_report.functions.contains_key("counter()"));
    assert!(sample_contract_report
        .functions
        .contains_key("found_needle()"));
    assert!(sample_contract_report
        .functions
        .contains_key("incrementBy(uint256)"));

    let increment_by_reports = sample_contract_report
        .functions
        .get("incrementBy(uint256)")
        .unwrap();

    assert!(!increment_by_reports.is_empty());
    assert!(increment_by_reports.iter().all(|r| r.gas > 0));
}

#[tokio::test(flavor = "multi_thread")]
async fn test_should_not_shrink_fuzz_failure() {
    let filter = SolidityTestFilter::new(".*", "FuzzFailureShrinkTest", ".*fuzz/");
    let mut config = TEST_DATA_DEFAULT.config_with_mock_rpc();
    config.fuzz.runs = 256;
    config.fuzz.seed = Some(U256::from(100));
    // The number of runs before the failure depends on worker scheduling.
    config.fuzz.workers = Some(1);
    let runner = TEST_DATA_DEFAULT.runner_with_fuzz_persistence(config).await;
    let suite_results = runner.test_collect(filter).await.suite_results;
    let suite_result = suite_results
        .get("default/fuzz/FuzzFailureShrink.t.sol:FuzzFailureShrinkTest")
        .unwrap();
    let test_result = suite_result
        .test_results
        .get("testAddOne(uint256)")
        .unwrap();
    assert_eq!(test_result.status, TestStatus::Failure);
    let TestKind::Fuzz { runs, .. } = test_result.kind else {
        panic!("not a fuzz test: {:?}", test_result.kind);
    };
    assert_eq!(runs, 27);
}

#[tokio::test(flavor = "multi_thread")]
async fn test_fuzz_can_scrape_bytecode() {
    let filter = SolidityTestFilter::new(".*", ".*", ".*fuzz/FuzzerDict.t.sol");
    let mut config = TEST_DATA_DEFAULT.config_with_mock_rpc();
    config.fuzz.runs = 2100;
    config.fuzz.seed = Some(U256::from(107u32));
    // Keep the seeded input stream independent of the number of threads.
    config.fuzz.workers = Some(1);
    let runner = TEST_DATA_DEFAULT.runner_with_fuzz_persistence(config).await;
    let results = runner.test_collect(filter).await.suite_results;

    assert_multiple(
        &results,
        BTreeMap::from([(
            "default/fuzz/FuzzerDict.t.sol:FuzzerDictTest",
            vec![
                ("testImmutableOwner(address)", false, None, None, None),
                ("testStorageOwner(address)", false, None, None, None),
            ],
        )]),
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn test_fuzz_show_logs() {
    let filter = SolidityTestFilter::new("testSuccessfulFuzz", ".*", ".*fuzz/Fuzz.t.sol");

    // With show_logs enabled, decoded_logs should contain the fuzz test's log
    // messages.
    let mut config = TEST_DATA_DEFAULT.config_with_mock_rpc();
    config.fuzz.show_logs = true;

    let runner = TEST_DATA_DEFAULT.runner_with_fuzz_persistence(config).await;
    let suite_result = runner.test_collect(filter.clone()).await.suite_results;

    for SuiteResult { test_results, .. } in suite_result.values() {
        let result = test_results
            .get("testSuccessfulFuzz(uint128,uint128)")
            .expect("test result should exist");
        assert_eq!(result.status, TestStatus::Success);
        assert!(
            result
                .decoded_logs
                .iter()
                .any(|log| log.contains("testSuccessfulFuzz")),
            "Expected logs to contain 'testSuccessfulFuzz' when show_logs is true, got: {:?}",
            result.decoded_logs,
        );
    }

    // With show_logs disabled (default), decoded_logs from successful fuzz runs
    // should not be collected.
    let mut config = TEST_DATA_DEFAULT.config_with_mock_rpc();
    config.fuzz.show_logs = false;

    let runner = TEST_DATA_DEFAULT.runner_with_fuzz_persistence(config).await;
    let suite_result = runner.test_collect(filter).await.suite_results;

    for SuiteResult { test_results, .. } in suite_result.values() {
        let result = test_results
            .get("testSuccessfulFuzz(uint128,uint128)")
            .expect("test result should exist");
        assert_eq!(result.status, TestStatus::Success);
        assert!(
            !result
                .decoded_logs
                .iter()
                .any(|log| log.contains("testSuccessfulFuzz")),
            "Expected no 'testSuccessfulFuzz' logs when show_logs is false, got: {:?}",
            result.decoded_logs,
        );
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn test_fuzz_timeout() {
    let filter = SolidityTestFilter::new(".*", ".*", ".*fuzz/FuzzTimeout.t.sol");
    let mut config = TEST_DATA_DEFAULT.config_with_mock_rpc();
    // Disable the reject limit so that the test can only end by timing out.
    config.fuzz.max_test_rejects = 0;
    config.fuzz.timeout = Some(1u32);
    let runner = TEST_DATA_DEFAULT.runner_with_fuzz_persistence(config).await;
    let results = runner.test_collect(filter).await.suite_results;

    assert_multiple(
        &results,
        BTreeMap::from([(
            "default/fuzz/FuzzTimeout.t.sol:FuzzTimeoutTest",
            vec![("test_fuzz_bound(uint256)", true, None, None, None)],
        )]),
    );
}

// Test 256 runs regardless number of test rejects.
// <https://github.com/foundry-rs/foundry/issues/9054>
#[tokio::test(flavor = "multi_thread")]
async fn test_fuzz_runs_with_rejects() {
    let filter = SolidityTestFilter::new(".*", ".*", ".*fuzz/FuzzWithRejects.t.sol");
    let mut config = TEST_DATA_DEFAULT.config_with_mock_rpc();
    config.fuzz.runs = 256;
    let runner = TEST_DATA_DEFAULT.runner_with_fuzz_persistence(config).await;
    let results = runner.test_collect(filter).await.suite_results;

    let result = results
        .get("default/fuzz/FuzzWithRejects.t.sol:FuzzWithRejectsTest")
        .unwrap()
        .test_results
        .get("testFuzzWithRejects(uint256)")
        .unwrap();
    assert_eq!(result.status, TestStatus::Success);
    let TestKind::Fuzz { runs, .. } = result.kind else {
        panic!("not a fuzz test: {:?}", result.kind);
    };
    assert_eq!(runs, 256);
}

/// Runs the `FuzzWithRejects` fixture with `counterexample` persisted for its
/// test and returns the number of runs of the (passing) test.
async fn fuzz_runs_with_persisted_failure(counterexample: &BaseCounterExample) -> usize {
    let filter = SolidityTestFilter::new(".*", ".*", ".*fuzz/FuzzWithRejects.t.sol");
    let persist_dir = tempfile::tempdir().unwrap();
    let failure_dir = persist_dir
        .path()
        .join("testfailure")
        .join("FuzzWithRejectsTest");
    std::fs::create_dir_all(&failure_dir).unwrap();
    std::fs::write(
        failure_dir.join("testFuzzWithRejects"),
        serde_json::to_vec(counterexample).unwrap(),
    )
    .unwrap();

    let mut config = TEST_DATA_DEFAULT.config_with_mock_rpc();
    config.fuzz.runs = 256;
    config.fuzz.failure_persist_dir = Some(persist_dir.path().to_path_buf());
    let runner = TEST_DATA_DEFAULT.runner_with_config(config).await;
    let results = runner.test_collect(filter).await.suite_results;
    let result = results
        .get("default/fuzz/FuzzWithRejects.t.sol:FuzzWithRejectsTest")
        .unwrap()
        .test_results
        .get("testFuzzWithRejects(uint256)")
        .unwrap();
    assert_eq!(result.status, TestStatus::Success, "{:?}", result.reason);
    let TestKind::Fuzz { runs, .. } = result.kind else {
        panic!("not a fuzz test: {:?}", result.kind);
    };
    runs
}

// Test that a persisted counterexample is only replayed if it targets the same
// test selector, and that a replayed input does not count as a run.
// <https://github.com/foundry-rs/foundry/issues/11927>
#[tokio::test(flavor = "multi_thread")]
async fn test_fuzz_replay_only_with_same_selector() {
    let arg = alloy_primitives::U256::from(2_000_000u32).to_be_bytes::<32>();

    // A counterexample recorded for a different function signature is ignored.
    let other_selector = &alloy_primitives::keccak256("testFuzzWithRejects(uint8)")[..4];
    let calldata = Bytes::from([other_selector, arg.as_slice()].concat());
    let counterexample = BaseCounterExample::from_fuzz_call(calldata, &[], None, None);
    assert_eq!(fuzz_runs_with_persisted_failure(&counterexample).await, 256);

    // A counterexample for the current signature is replayed first. Its input
    // is rejected by `vm.assume`, which must not eat into the configured runs.
    let selector = &alloy_primitives::keccak256("testFuzzWithRejects(uint256)")[..4];
    let calldata = Bytes::from([selector, arg.as_slice()].concat());
    let counterexample = BaseCounterExample::from_fuzz_call(calldata, &[], None, None);
    assert_eq!(fuzz_runs_with_persisted_failure(&counterexample).await, 256);
}

// Tests that `vm.randomUint()` produces different values across fuzz runs.
// Regression test for <https://github.com/foundry-rs/foundry/issues/12817>
//
// The issue was that `vm.randomUint()` would produce the same sequence of
// values in every fuzz run because the RNG was seeded identically for each
// run. This test verifies that with many fuzz runs and a small range, we
// eventually hit value 0, which proves the RNG varies across runs.
#[tokio::test(flavor = "multi_thread")]
async fn test_fuzz_random_uint_varies_across_runs() {
    let filter = SolidityTestFilter::new(".*", ".*", ".*fuzz/RandomFuzz.t.sol");
    let mut config = TEST_DATA_DEFAULT.config_with_mock_rpc();
    config.fuzz.seed = Some(U256::from(1u32));
    let runner = TEST_DATA_DEFAULT.runner_with_fuzz_persistence(config).await;
    let results = runner.test_collect(filter).await.suite_results;

    assert_multiple(
        &results,
        BTreeMap::from([(
            "default/fuzz/RandomFuzz.t.sol:RandomFuzzTest",
            // `DSTest::assertTrue` logs the message instead of reverting.
            vec![(
                "testFuzz_randomUint_shouldFail(uint256)",
                false,
                None,
                None,
                None,
            )],
        )]),
    );
}

const FUZZ_WITH_REJECTS: &str = "default/fuzz/FuzzWithRejects.t.sol:FuzzWithRejectsTest";
const FUZZ_WITH_REJECTS_TEST: &str = "testFuzzWithRejects(uint256)";
const FUZZ_FAILURE_PERSIST: &str = "default/fuzz/FuzzFailurePersist.t.sol:FuzzFailurePersistTest";
const FUZZ_FAILURE_PERSIST_TEST: &str =
    "test_persist_fuzzed_failure(uint256,int256,address,bool,string,(address,uint256),address[])";

/// Returns the status and `(runs, mean_gas, median_gas)` of a fuzz test.
macro_rules! fuzz_outcome {
    ($results:expr, $contract:expr, $test_name:expr) => {{
        let result = $results
            .get($contract)
            .unwrap()
            .test_results
            .get($test_name)
            .unwrap();
        let TestKind::Fuzz {
            runs,
            mean_gas,
            median_gas,
        } = result.kind
        else {
            panic!("not a fuzz test: {:?}", result.kind);
        };
        (result.status, (runs, mean_gas, median_gas))
    }};
}

#[tokio::test(flavor = "multi_thread")]
async fn test_fuzz_parallel_workers_run_all_runs() {
    let filter = SolidityTestFilter::new(".*", ".*", ".*fuzz/FuzzWithRejects.t.sol");
    let mut config = TEST_DATA_DEFAULT.config_with_mock_rpc();
    config.fuzz.runs = 1000;
    config.fuzz.workers = Some(4);
    let runner = TEST_DATA_DEFAULT.runner_with_fuzz_persistence(config).await;
    let results = runner.test_collect(filter).await.suite_results;

    let (status, (runs, _, _)) = fuzz_outcome!(results, FUZZ_WITH_REJECTS, FUZZ_WITH_REJECTS_TEST);
    assert_eq!(status, TestStatus::Success);
    assert_eq!(runs, 1000);
}

#[tokio::test(flavor = "multi_thread")]
async fn test_fuzz_parallel_workers_report_failure() {
    let filter = SolidityTestFilter::new(".*", ".*", ".*fuzz/FuzzFailurePersist.t.sol");
    let mut config = TEST_DATA_DEFAULT.config_with_mock_rpc();
    config.fuzz.runs = 1000;
    config.fuzz.workers = Some(4);
    let runner = TEST_DATA_DEFAULT.runner_with_fuzz_persistence(config).await;
    let results = runner.test_collect(filter).await.suite_results;

    let result = results
        .get(FUZZ_FAILURE_PERSIST)
        .unwrap()
        .test_results
        .get(FUZZ_FAILURE_PERSIST_TEST)
        .unwrap();
    assert_eq!(result.status, TestStatus::Failure);
    assert!(matches!(
        result.counterexample,
        Some(CounterExample::Single(_))
    ));
    let (_, (runs, _, _)) = fuzz_outcome!(results, FUZZ_FAILURE_PERSIST, FUZZ_FAILURE_PERSIST_TEST);
    assert!(runs <= 1000, "{runs}");
}

/// The same seed and worker count must produce the same inputs on every run.
#[tokio::test(flavor = "multi_thread")]
async fn test_fuzz_parallel_workers_are_deterministic() {
    let filter = SolidityTestFilter::new(".*", ".*", ".*fuzz/FuzzWithRejects.t.sol");
    let mut outcomes = Vec::new();
    for _ in 0..2 {
        let mut config = TEST_DATA_DEFAULT.config_with_mock_rpc();
        config.fuzz.runs = 512;
        config.fuzz.seed = Some(U256::from(1234u32));
        config.fuzz.workers = Some(2);
        let runner = TEST_DATA_DEFAULT.runner_with_fuzz_persistence(config).await;
        let results = runner.test_collect(filter.clone()).await.suite_results;
        outcomes.push(fuzz_outcome!(
            results,
            FUZZ_WITH_REJECTS,
            FUZZ_WITH_REJECTS_TEST
        ));
    }
    assert_eq!(outcomes[0].0, TestStatus::Success);
    assert_eq!(outcomes[0].1 .0, 512);
    assert_eq!(outcomes[0], outcomes[1]);
}

#[tokio::test(flavor = "multi_thread")]
async fn test_fuzz_zero_runs() {
    let filter = SolidityTestFilter::new(".*", ".*", ".*fuzz/FuzzFailurePersist.t.sol");
    let mut config = TEST_DATA_DEFAULT.config_with_mock_rpc();
    config.fuzz.runs = 0;
    let runner = TEST_DATA_DEFAULT.runner_with_fuzz_persistence(config).await;
    let results = runner.test_collect(filter).await.suite_results;

    let (status, (runs, _, _)) =
        fuzz_outcome!(results, FUZZ_FAILURE_PERSIST, FUZZ_FAILURE_PERSIST_TEST);
    assert_eq!(status, TestStatus::Success);
    assert_eq!(runs, 0);
}

#[tokio::test(flavor = "multi_thread")]
async fn test_fuzz_fail_on_revert() {
    let filter = SolidityTestFilter::new(".*", ".*", ".*fuzz/FuzzFailOnRevert.t.sol");
    let mut config = TEST_DATA_DEFAULT.config_with_mock_rpc();
    config.fuzz.fail_on_revert = false;
    let runner = TEST_DATA_DEFAULT.runner_with_fuzz_persistence(config).await;
    let results = runner.test_collect(filter).await.suite_results;

    assert_multiple(
        &results,
        BTreeMap::from([
            (
                "default/fuzz/FuzzFailOnRevert.t.sol:CounterTest",
                vec![
                    ("testFuzz_SetNumberRequire(uint256)", true, None, None, None),
                    ("testFuzz_SetNumberAssert(uint256)", true, None, None, None),
                ],
            ),
            (
                "default/fuzz/FuzzFailOnRevert.t.sol:AnotherCounterTest",
                vec![
                    (
                        "testFuzz_SetNumberRequire(uint256)",
                        false,
                        Some("EvmError: Revert".into()),
                        None,
                        None,
                    ),
                    ("testFuzz_SetNumberAssert(uint256)", false, None, None, None),
                ],
            ),
        ]),
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn test_fuzz_function_overrides() {
    let filter = SolidityTestFilter::new(".*", ".*", ".*fuzz/FuzzConfigOverride.t.sol");
    let mut config = TEST_DATA_DEFAULT.config_with_mock_rpc();
    config.fuzz.runs = 100;
    config.fuzz.max_test_rejects = 1;

    // Per-function overrides come from inline `forge-config:` directives in
    // `fuzz/FuzzConfigOverride.t.sol`.
    let runner = TEST_DATA_DEFAULT.runner_with_fuzz_persistence(config).await;
    let results = runner.test_collect(filter).await.suite_results;

    assert_multiple(
        &results,
        BTreeMap::from([(
            "default/fuzz/FuzzConfigOverride.t.sol:FuzzConfigOverrideTest",
            vec![
                ("testFuzz_OverrideRuns(uint256)", true, None, None, None),
                ("testFuzz_NoOverrideRuns(uint256)", true, None, None, None),
                (
                    "testFuzz_OverrideTimeoutAndRejects(uint256)",
                    true,
                    None,
                    None,
                    None,
                ),
                (
                    "testFuzz_NoOverrideTimeout(uint256)",
                    false,
                    Some("`vm.assume` rejected too many inputs (5000 allowed)".into()),
                    None,
                    None,
                ),
                (
                    "testFuzz_NoOverrideRejects(uint256)",
                    false,
                    Some("`vm.assume` rejected too many inputs (1 allowed)".into()),
                    None,
                    None,
                ),
            ],
        )]),
    );

    let suite_result = results
        .get("default/fuzz/FuzzConfigOverride.t.sol:FuzzConfigOverrideTest")
        .unwrap();

    let default_runs_result = suite_result
        .test_results
        .get("testFuzz_NoOverrideRuns(uint256)")
        .unwrap();
    assert!(matches!(
        default_runs_result.kind,
        TestKind::Fuzz { runs: 100, .. }
    ));

    let override_runs_result = suite_result
        .test_results
        .get("testFuzz_OverrideRuns(uint256)")
        .unwrap();
    assert!(matches!(
        override_runs_result.kind,
        TestKind::Fuzz { runs: 10, .. }
    ));
}

/// A prefixed directive applies only under its own profile; an unprefixed one
/// applies under every profile. See `fuzz/FuzzProfileOverride.t.sol`.
#[tokio::test(flavor = "multi_thread")]
async fn test_fuzz_profile_overrides() {
    const GLOBAL_RUNS: usize = 100;

    // The run counts each test resolves to under each selected profile. A test
    // with no directive that applies lands on the global config's `GLOBAL_RUNS`.
    let expected_runs = [
        (
            "default",
            [
                ("testFuzz_Unprefixed", 3),
                ("testFuzz_ProfileWinsOverUnprefixed", 3),
                ("testFuzz_DefaultProfileOnly", 4),
            ],
        ),
        (
            "ci",
            [
                ("testFuzz_Unprefixed", 3),
                ("testFuzz_ProfileWinsOverUnprefixed", 8),
                ("testFuzz_DefaultProfileOnly", GLOBAL_RUNS),
            ],
        ),
    ];

    for (selected, cases) in expected_runs {
        let filter = SolidityTestFilter::new(".*", ".*", ".*fuzz/FuzzProfileOverride.t.sol");
        let mut config = TEST_DATA_DEFAULT.config_with_mock_rpc();
        config.fuzz.runs = u32::try_from(GLOBAL_RUNS).expect("runs fit in u32");
        config.fuzz.max_test_rejects = 0;
        config.inline_config_profiles =
            InlineConfigProfiles::new(selected, ["ci".to_owned()]).expect("valid profiles");

        let runner = TEST_DATA_DEFAULT.runner_with_fuzz_persistence(config).await;
        let results = runner.test_collect(filter).await.suite_results;

        let suite_result = results
            .get("default/fuzz/FuzzProfileOverride.t.sol:FuzzProfileOverrideTest")
            .unwrap_or_else(|| panic!("suite ran under {selected}"));

        for (function, expected) in cases {
            let name = format!("{function}(uint256)");
            let result = suite_result
                .test_results
                .get(&name)
                .unwrap_or_else(|| panic!("{name} ran under {selected}"));

            let TestKind::Fuzz { runs, .. } = result.kind else {
                panic!(
                    "{name} under {selected} is not a fuzz test: {:?}",
                    result.kind
                );
            };
            assert_eq!(runs, expected, "{name} under profile `{selected}`");
        }
    }
}
