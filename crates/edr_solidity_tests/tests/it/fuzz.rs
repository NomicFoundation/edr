//! Fuzz tests.

use std::collections::BTreeMap;

use alloy_primitives::{Bytes, U256};
use edr_gas_report::GasReportExecutionStatus;
use edr_solidity_tests::{
    fuzz::CounterExample,
    result::{SuiteResult, TestStatus},
};

use crate::helpers::{assert_multiple, SolidityTestFilter, TestFuzzConfig, TEST_DATA_DEFAULT};

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
            ]
            .join("|"),
        )
        .exclude_paths("invariant");
    let runner = TEST_DATA_DEFAULT.runner().await;
    let suite_result = runner.test_collect(filter).await.suite_results;

    assert!(!suite_result.is_empty());

    for (_, SuiteResult { test_results, .. }) in suite_result {
        for (test_name, result) in test_results {
            match test_name.as_str() {
                "testPositive(uint256)"
                | "testPositive(int256)"
                | "testSuccessfulFuzz(uint128,uint128)"
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
    let runner = TEST_DATA_DEFAULT.runner_with_config(config).await;
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
    let mut fuzz_config = TestFuzzConfig {
        runs: 1000,
        ..TestFuzzConfig::default()
    };
    let runner = TEST_DATA_DEFAULT
        .runner_with_fuzz_config(fuzz_config.clone())
        .await;

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
    fuzz_config.failure_persist_file = "failure1".to_string();
    let runner = TEST_DATA_DEFAULT.runner_with_fuzz_config(fuzz_config).await;
    let new_calldata = match get_failure_result!(runner) {
        Some(CounterExample::Single(counterexample)) => counterexample.calldata,
        _ => Bytes::new(),
    };
    // empty file is used to load failure so new calldata is generated
    assert_ne!(initial_calldata, new_calldata);
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
    let runner = TEST_DATA_DEFAULT.runner_with_config(config).await;
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
async fn test_fuzz_can_scrape_bytecode() {
    let filter = SolidityTestFilter::new(".*", ".*", ".*fuzz/FuzzerDict.t.sol");
    let mut config = TEST_DATA_DEFAULT.config_with_mock_rpc();
    config.fuzz.runs = 2100;
    config.fuzz.seed = Some(U256::from(119u32));
    let runner = TEST_DATA_DEFAULT.runner_with_config(config).await;
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
async fn test_fuzz_timeout() {
    let filter = SolidityTestFilter::new(".*", ".*", ".*fuzz/FuzzTimeout.t.sol");
    let mut config = TEST_DATA_DEFAULT.config_with_mock_rpc();
    config.fuzz.max_test_rejects = 50000;
    config.fuzz.timeout = Some(1u32);
    let runner = TEST_DATA_DEFAULT.runner_with_config(config).await;
    let results = runner.test_collect(filter).await.suite_results;

    assert_multiple(
        &results,
        BTreeMap::from([(
            "default/fuzz/FuzzTimeout.t.sol:FuzzTimeoutTest",
            vec![("test_fuzz_bound(uint256)", true, None, None, None)],
        )]),
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn test_fuzz_fail_on_revert() {
    let filter = SolidityTestFilter::new(".*", ".*", ".*fuzz/FuzzFailOnRevert.t.sol");
    let mut config = TEST_DATA_DEFAULT.config_with_mock_rpc();
    config.fuzz.fail_on_revert = false;
    let runner = TEST_DATA_DEFAULT.runner_with_config(config).await;
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
