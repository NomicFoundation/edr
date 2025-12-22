//! Forge tests for core functionality.

use std::{
    collections::{BTreeMap, HashMap},
    env,
};

use edr_gas_report::GasReportExecutionStatus;
use edr_solidity_tests::{
    multi_runner::SolidityTestsRunResult,
    result::{SuiteResult, TestStatus},
};
use foundry_evm::traces::TraceKind;

use crate::helpers::{assert_multiple, SolidityTestFilter, TEST_DATA_DEFAULT, TEST_DATA_PARIS};

#[tokio::test(flavor = "multi_thread")]
async fn test_core() {
    let filter = SolidityTestFilter::new(".*", ".*", ".*core");
    let runner = TEST_DATA_DEFAULT.runner().await;
    let SolidityTestsRunResult {
        test_result: _,
        suite_results,
    } = runner.test_collect(filter).await;

    assert_multiple(
        &suite_results,
        BTreeMap::from([
            (
                "default/core/FailingSetup.t.sol:FailingSetupTest",
                vec![(
                    "setUp()",
                    false,
                    Some("setup failed predictably".to_string()),
                    None,
                    None,
                )],
            ),
            (
                "default/core/MultipleSetup.t.sol:MultipleSetup",
                vec![(
                    "setUp()",
                    false,
                    Some("multiple setUp functions".to_string()),
                    None,
                    Some(1),
                )],
            ),
            (
                "default/core/Reverting.t.sol:RevertingTest",
                vec![("testRevert()", true, None, None, None)],
            ),
            (
                "default/core/InternalRevert.t.sol:InternalRevertingTest",
                vec![(
                    "testInternalRevert()",
                    false,
                    Some(
                        "call didn't revert at a lower depth than cheatcode call depth".to_string(),
                    ),
                    None,
                    None,
                )],
            ),
            (
                "default/core/SetupConsistency.t.sol:SetupConsistencyCheck",
                vec![
                    ("testAdd()", true, None, None, None),
                    ("testMultiply()", true, None, None, None),
                ],
            ),
            (
                "default/core/DSStyle.t.sol:DSStyleTest",
                vec![("testDSTestFailingAssertions()", false, None, None, None)],
            ),
            (
                "default/core/ContractEnvironment.t.sol:ContractEnvironmentTest",
                vec![
                    ("testAddresses()", true, None, None, None),
                    ("testEnvironment()", true, None, None, None),
                ],
            ),
            (
                "default/core/PaymentFailure.t.sol:PaymentFailureTest",
                vec![(
                    "testCantPay()",
                    false,
                    Some("EvmError: Revert".to_string()),
                    None,
                    None,
                )],
            ),
            (
                "default/core/Abstract.t.sol:AbstractTest",
                vec![("testSomething()", true, None, None, None)],
            ),
            (
                "default/core/FailingTestAfterFailedSetup.t.sol:FailingTestAfterFailedSetupTest",
                vec![(
                    "setUp()",
                    false,
                    Some("execution error".to_string()),
                    None,
                    None,
                )],
            ),
            (
                "default/core/BadSigAfterInvariant.t.sol:BadSigAfterInvariant",
                vec![("testShouldPassWithWarning()", true, None, None, None)],
            ),
            (
                "default/core/ExecutionContext.t.sol:ExecutionContextTest",
                vec![("testContext()", true, None, None, None)],
            ),
            (
                "default/core/DeprecatedCheatcode.t.sol:DeprecatedCheatcodeTest",
                vec![("test_deprecated_cheatcode()", true, None, None, None)],
            ),
            (
                "default/core/DeprecatedCheatcode.t.sol:DeprecatedCheatcodeFuzzTest",
                vec![("test_deprecated_cheatcode(uint256)", true, None, None, None)],
            ),
            (
                "default/core/DeprecatedCheatcode.t.sol:DeprecatedCheatcodeInvariantTest",
                vec![("invariant_deprecated_cheatcode()", true, None, None, None)],
            ),
        ]),
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn test_linking() {
    let filter = SolidityTestFilter::new(".*", ".*", ".*linking");
    let runner = TEST_DATA_DEFAULT.runner().await;
    let results = runner.test_collect(filter).await.suite_results;

    assert_multiple(
        &results,
        BTreeMap::from([
            (
                "default/linking/simple/Simple.t.sol:SimpleLibraryLinkingTest",
                vec![("testCall()", true, None, None, None)],
            ),
            (
                "default/linking/nested/Nested.t.sol:NestedLibraryLinkingTest",
                vec![
                    ("testDirect()", true, None, None, None),
                    ("testNested()", true, None, None, None),
                ],
            ),
            (
                "default/linking/duplicate/Duplicate.t.sol:DuplicateLibraryLinkingTest",
                vec![
                    ("testA()", true, None, None, None),
                    ("testB()", true, None, None, None),
                    ("testC()", true, None, None, None),
                    ("testD()", true, None, None, None),
                    ("testE()", true, None, None, None),
                ],
            ),
        ]),
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn test_logs() {
    let filter = SolidityTestFilter::new(".*", ".*", ".*logs");
    let runner = TEST_DATA_DEFAULT.runner().await;
    let results = runner.test_collect(filter).await.suite_results;

    assert_multiple(
        &results,
        BTreeMap::from([
            (
                "default/logs/DebugLogs.t.sol:DebugLogsTest",
                vec![
                    (
                        "test1()",
                        true,
                        None,
                        Some(vec!["0".into(), "1".into(), "2".into()]),
                        None,
                    ),
                    (
                        "test2()",
                        true,
                        None,
                        Some(vec!["0".into(), "1".into(), "3".into()]),
                        None,
                    ),
                    (
                        "testRevertIfWithRequire()",
                        true,
                        None,
                        Some(vec!["0".into(), "1".into(), "5".into()]),
                        None,
                    ),
                    (
                        "testRevertIfWithRevert()",
                        true,
                        None,
                        Some(vec!["0".into(), "1".into(), "4".into(), "100".into()]),
                        None,
                    ),
                    (
                        "testLog()",
                        true,
                        None,
                        Some(vec!["0".into(), "1".into(), "Error: Assertion Failed".into()]),
                        None,
                    ),
                    (
                        "testLogs()",
                        true,
                        None,
                        Some(vec!["0".into(), "1".into(), "0x61626364".into()]),
                        None,
                    ),
                    (
                        "testLogAddress()",
                        true,
                        None,
                        Some(vec![
                            "0".into(),
                            "1".into(),
                            "0x0000000000000000000000000000000000000001".into(),
                        ]),
                        None,
                    ),
                    (
                        "testLogBytes32()",
                        true,
                        None,
                        Some(vec![
                            "0".into(),
                            "1".into(),
                            "0x6162636400000000000000000000000000000000000000000000000000000000".into()]),
                        None,
                    ),
                    (
                        "testLogInt()",
                        true,
                        None,
                        Some(vec!["0".into(), "1".into(), "-31337".into()]),
                        None,
                    ),
                    (
                        "testLogBytes()",
                        true,
                        None,
                        Some(vec!["0".into(), "1".into(), "0x61626364".into()]),
                        None,
                    ),
                    (
                        "testLogString()",
                        true,
                        None,
                        Some(vec!["0".into(), "1".into(), "here".into()]),
                        None,
                    ),
                    (
                        "testLogNamedAddress()",
                        true,
                        None,
                        Some(vec![
                            "0".into(),
                            "1".into(),
                            "address: 0x0000000000000000000000000000000000000001".into()]),
                        None,
                    ),
                    (
                        "testLogNamedBytes32()",
                        true,
                        None,
                        Some(vec![
                            "0".into(),
                            "1".into(),
                            "abcd: 0x6162636400000000000000000000000000000000000000000000000000000000".into()]),
                        None,
                    ),
                    (
                        "testLogNamedDecimalInt()",
                        true,
                        None,
                        Some(vec![
                            "0".into(),
                            "1".into(),
                            "amount: -0.000000000000031337".into()]),
                        None,
                    ),
                    (
                        "testLogNamedDecimalUint()",
                        true,
                        None,
                        Some(vec![
                            "0".into(),
                            "1".into(),
                            "amount: 1.000000000000000000".into()]),
                        None,
                    ),
                    (
                        "testLogNamedInt()",
                        true,
                        None,
                        Some(vec![
                            "0".into(),
                            "1".into(),
                            "amount: -31337".into()]),
                        None,
                    ),
                    (
                        "testLogNamedUint()",
                        true,
                        None,
                        Some(vec![
                            "0".into(),
                            "1".into(),
                            "amount: 1000000000000000000".into()]),
                        None,
                    ),
                    (
                        "testLogNamedBytes()",
                        true,
                        None,
                        Some(vec![
                            "0".into(),
                            "1".into(),
                            "abcd: 0x61626364".into()]),
                        None,
                    ),
                    (
                        "testLogNamedString()",
                        true,
                        None,
                        Some(vec![
                            "0".into(),
                            "1".into(),
                            "key: val".into()]),
                        None,
                    ),
                ],
            ),
            (
                "default/logs/HardhatLogs.t.sol:HardhatLogsTest",
                vec![
                    (
                        "testInts()",
                        true,
                        None,
                        Some(vec![
                            "constructor".into(),
                            "0".into(),
                            "1".into(),
                            "2".into(),
                            "3".into(),
                        ]),
                        None,
                    ),
                    (
                        "testMisc()",
                        true,
                        None,
                        Some(vec![
                            "constructor".into(),
                            "testMisc 0x0000000000000000000000000000000000000001".into(),
                            "testMisc 42".into(),
                        ]),
                        None,
                    ),
                    (
                        "testStrings()",
                        true,
                        None,
                        Some(vec!["constructor".into(), "testStrings".into()]),
                        None,
                    ),
                    (
                        "testConsoleLog()",
                        true,
                        None,
                        Some(vec!["constructor".into(), "test".into()]),
                        None,
                    ),
                    (
                        "testLogInt()",
                        true,
                        None,
                        Some(vec!["constructor".into(), "-31337".into()]),
                        None,
                    ),
                    (
                        "testLogUint()",
                        true,
                        None,
                        Some(vec!["constructor".into(), "1".into()]),
                        None,
                    ),
                    (
                        "testLogString()",
                        true,
                        None,
                        Some(vec!["constructor".into(), "test".into()]),
                        None,
                    ),
                    (
                        "testLogBool()",
                        true,
                        None,
                        Some(vec!["constructor".into(), "false".into()]),
                        None,
                    ),
                    (
                        "testLogAddress()",
                        true,
                        None,
                        Some(vec!["constructor".into(), "0x0000000000000000000000000000000000000001".into()]),
                        None,
                    ),
                    (
                        "testLogBytes()",
                        true,
                        None,
                        Some(vec!["constructor".into(), "0x61".into()]),
                        None,
                    ),
                    (
                        "testLogBytes1()",
                        true,
                        None,
                        Some(vec!["constructor".into(), "0x61".into()]),
                        None,
                    ),
                    (
                        "testLogBytes2()",
                        true,
                        None,
                        Some(vec!["constructor".into(), "0x6100".into()]),
                        None,
                    ),
                    (
                        "testLogBytes3()",
                        true,
                        None,
                        Some(vec!["constructor".into(), "0x610000".into()]),
                        None,
                    ),
                    (
                        "testLogBytes4()",
                        true,
                        None,
                        Some(vec!["constructor".into(), "0x61000000".into()]),
                        None,
                    ),
                    (
                        "testLogBytes5()",
                        true,
                        None,
                        Some(vec!["constructor".into(), "0x6100000000".into()]),
                        None,
                    ),
                    (
                        "testLogBytes6()",
                        true,
                        None,
                        Some(vec!["constructor".into(), "0x610000000000".into()]),
                        None,
                    ),
                    (
                        "testLogBytes7()",
                        true,
                        None,
                        Some(vec!["constructor".into(), "0x61000000000000".into()]),
                        None,
                    ),
                    (
                        "testLogBytes8()",
                        true,
                        None,
                        Some(vec!["constructor".into(), "0x6100000000000000".into()]),
                        None,
                    ),
                    (
                        "testLogBytes9()",
                        true,
                        None,
                        Some(vec!["constructor".into(), "0x610000000000000000".into()]),
                        None,
                    ),
                    (
                        "testLogBytes10()",
                        true,
                        None,
                        Some(vec!["constructor".into(), "0x61000000000000000000".into()]),
                        None,
                    ),
                    (
                        "testLogBytes11()",
                        true,
                        None,
                        Some(vec!["constructor".into(), "0x6100000000000000000000".into()]),
                        None,
                    ),
                    (
                        "testLogBytes12()",
                        true,
                        None,
                        Some(vec!["constructor".into(), "0x610000000000000000000000".into()]),
                        None,
                    ),
                    (
                        "testLogBytes13()",
                        true,
                        None,
                        Some(vec!["constructor".into(), "0x61000000000000000000000000".into()]),
                        None,
                    ),
                    (
                        "testLogBytes14()",
                        true,
                        None,
                        Some(vec!["constructor".into(), "0x6100000000000000000000000000".into()]),
                        None,
                    ),
                    (
                        "testLogBytes15()",
                        true,
                        None,
                        Some(vec!["constructor".into(), "0x610000000000000000000000000000".into()]),
                        None,
                    ),
                    (
                        "testLogBytes16()",
                        true,
                        None,
                        Some(vec!["constructor".into(), "0x61000000000000000000000000000000".into()]),
                        None,
                    ),
                    (
                        "testLogBytes17()",
                        true,
                        None,
                        Some(vec!["constructor".into(), "0x6100000000000000000000000000000000".into()]),
                        None,
                    ),
                    (
                        "testLogBytes18()",
                        true,
                        None,
                        Some(vec!["constructor".into(), "0x610000000000000000000000000000000000".into()]),
                        None,
                    ),
                    (
                        "testLogBytes19()",
                        true,
                        None,
                        Some(vec!["constructor".into(), "0x61000000000000000000000000000000000000".into()]),
                        None,
                    ),
                    (
                        "testLogBytes20()",
                        true,
                        None,
                        Some(vec!["constructor".into(), "0x6100000000000000000000000000000000000000".into()]),
                        None,
                    ),
                    (
                        "testLogBytes21()",
                        true,
                        None,
                        Some(vec!["constructor".into(), "0x610000000000000000000000000000000000000000".into()]),
                        None,
                    ),
                    (
                        "testLogBytes22()",
                        true,
                        None,
                        Some(vec!["constructor".into(), "0x61000000000000000000000000000000000000000000".into()]),
                        None,
                    ),
                    (
                        "testLogBytes23()",
                        true,
                        None,
                        Some(vec!["constructor".into(), "0x6100000000000000000000000000000000000000000000".into()]),
                        None,
                    ),
                    (
                        "testLogBytes24()",
                        true,
                        None,
                        Some(vec!["constructor".into(), "0x610000000000000000000000000000000000000000000000".into()]),
                        None,
                    ),
                    (
                        "testLogBytes25()",
                        true,
                        None,
                        Some(vec!["constructor".into(), "0x61000000000000000000000000000000000000000000000000".into()]),
                        None,
                    ),
                    (
                        "testLogBytes26()",
                        true,
                        None,
                        Some(vec!["constructor".into(), "0x6100000000000000000000000000000000000000000000000000".into()]),
                        None,
                    ),
                    (
                        "testLogBytes27()",
                        true,
                        None,
                        Some(vec!["constructor".into(), "0x610000000000000000000000000000000000000000000000000000".into()]),
                        None,
                    ),
                    (
                        "testLogBytes28()",
                        true,
                        None,
                        Some(vec!["constructor".into(), "0x61000000000000000000000000000000000000000000000000000000".into()]),
                        None,
                    ),
                    (
                        "testLogBytes29()",
                        true,
                        None,
                        Some(vec!["constructor".into(), "0x6100000000000000000000000000000000000000000000000000000000".into()]),
                        None,
                    ),
                    (
                        "testLogBytes30()",
                        true,
                        None,
                        Some(vec!["constructor".into(), "0x610000000000000000000000000000000000000000000000000000000000".into()]),
                        None,
                    ),
                    (
                        "testLogBytes31()",
                        true,
                        None,
                        Some(vec!["constructor".into(), "0x61000000000000000000000000000000000000000000000000000000000000".into()]),
                        None,
                    ),
                    (
                        "testLogBytes32()",
                        true,
                        None,
                        Some(vec!["constructor".into(), "0x6100000000000000000000000000000000000000000000000000000000000000".into()]),
                        None,
                    ),
                    (
                        "testConsoleLogUint()",
                        true,
                        None,
                        Some(vec!["constructor".into(), "1".into()]),
                        None,
                    ),
                    (
                        "testConsoleLogString()",
                        true,
                        None,
                        Some(vec!["constructor".into(), "test".into()]),
                        None,
                    ),
                    (
                        "testConsoleLogBool()",
                        true,
                        None,
                        Some(vec!["constructor".into(), "false".into()]),
                        None,
                    ),
                    (
                        "testConsoleLogAddress()",
                        true,
                        None,
                        Some(vec!["constructor".into(), "0x0000000000000000000000000000000000000001".into()]),
                        None,
                    ),
                    (
                        "testConsoleLogFormatString()",
                        true,
                        None,
                        Some(vec!["constructor".into(), "formatted log str=test".into()]),
                        None,
                    ),
                    (
                        "testConsoleLogFormatUint()",
                        true,
                        None,
                        Some(vec!["constructor".into(), "formatted log uint=1".into()]),
                        None,
                    ),
                    (
                        "testConsoleLogFormatAddress()",
                        true,
                        None,
                        Some(vec!["constructor".into(), "formatted log addr=0x0000000000000000000000000000000000000001".into()]),
                        None,
                    ),
                    (
                        "testConsoleLogFormatMulti()",
                        true,
                        None,
                        Some(vec!["constructor".into(), "formatted log str=test uint=1".into()]),
                        None,
                    ),
                    (
                        "testConsoleLogFormatEscape()",
                        true,
                        None,
                        Some(vec!["constructor".into(), "formatted log % test".into()]),
                        None,
                    ),
                    (
                        "testConsoleLogFormatSpill()",
                        true,
                        None,
                        Some(vec!["constructor".into(), "formatted log test 1".into()]),
                        None,
                    ),
                ],
            ),
        ]),
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn test_env_vars() {
    let env_var_key = "_foundryCheatcodeSetEnvTestKey";
    let env_var_val = "_foundryCheatcodeSetEnvTestVal";
    unsafe {
        env::remove_var(env_var_key);
    }

    let filter = SolidityTestFilter::new("testSetEnv", ".*", ".*");
    let runner = TEST_DATA_DEFAULT.runner().await;
    let _ = runner.test_collect(filter).await;

    assert_eq!(env::var(env_var_key).unwrap(), env_var_val);
}

#[tokio::test(flavor = "multi_thread")]
async fn test_doesnt_run_abstract_contract() {
    let filter = SolidityTestFilter::new(".*", ".*", ".*Abstract.t.sol".to_string().as_str());
    let runner = TEST_DATA_DEFAULT.runner().await;
    let results = runner.test_collect(filter).await.suite_results;
    assert!(!results.contains_key("default/core/Abstract.t.sol:AbstractTestBase"));
    assert!(results.contains_key("default/core/Abstract.t.sol:AbstractTest"));
}

#[tokio::test(flavor = "multi_thread")]
async fn test_trace() {
    let filter = SolidityTestFilter::new(".*", ".*", ".*trace");
    let runner = TEST_DATA_DEFAULT.tracing_runner().await;
    let suite_result = runner.test_collect(filter).await.suite_results;

    // TODO: This trace test is very basic - it is probably a good candidate for
    // snapshot testing.
    for (_, SuiteResult { test_results, .. }) in suite_result {
        for (test_name, result) in test_results {
            let deployment_traces = result
                .traces
                .iter()
                .filter(|(kind, _)| *kind == TraceKind::Deployment);
            let setup_traces = result
                .traces
                .iter()
                .filter(|(kind, _)| *kind == TraceKind::Setup);
            let execution_traces = result
                .traces
                .iter()
                .filter(|(kind, _)| *kind == TraceKind::Execution);
            assert_eq!(
                deployment_traces.count(),
                12, // includes libraries
                "Test {test_name} did not have exactly 12 deployment trace."
            );
            assert!(
                setup_traces.count() <= 1,
                "Test {test_name} had more than 1 setup trace."
            );
            assert_eq!(
                execution_traces.count(),
                1,
                "Test {test_name} did not not have exactly 1 execution trace."
            );
        }
    }
}

/// Test `beforeTest` functionality and `selfdestruct`.
/// See <https://github.com/foundry-rs/foundry/issues/1543>
#[tokio::test(flavor = "multi_thread")]
async fn test_before_setup_with_selfdestruct() {
    let filter = SolidityTestFilter::new(".*", ".*BeforeTestSelfDestructTest", ".*");
    let runner = TEST_DATA_PARIS.runner().await;
    let results = runner.test_collect(filter).await.suite_results;

    assert_multiple(
        &results,
        BTreeMap::from([(
            "paris/core/BeforeTest.t.sol:BeforeTestSelfDestructTest",
            vec![
                ("testKill()", true, None, None, None),
                ("testA()", true, None, None, None),
                ("testSimpleA()", true, None, None, None),
                ("testB()", true, None, None, None),
                ("testC(uint256)", true, None, None, None),
            ],
        )]),
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn test_deprecated_cheatcode_warning() {
    fn assert_multiple_deprecation_warnings(
        actuals: &BTreeMap<String, SuiteResult<edr_chain_l1::HaltReason>>,
        expecteds: BTreeMap<&str, Vec<&str>>,
    ) {
        const DEPRECATION_WARNING: &str = "The following cheatcode(s) are deprecated and will be removed in future versions:\n  keyExists(string,string): replaced by `keyExistsJson`";

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

            let suite_result = &actuals[*contract_name];
            assert_eq!(
                suite_result.len(),
                expecteds[contract_name].len(),
                "We did not run as many test functions as we expected for {contract_name}"
            );

            assert!(
                suite_result
                    .warnings
                    .contains(&DEPRECATION_WARNING.to_owned()),
                "We did not get the expected deprecation warning for contract {contract_name}: {:?}",
                suite_result.warnings
            );

            for test_name in tests {
                assert!(
                    suite_result.test_results.contains_key(*test_name),
                    "We did not run the test {test_name} in contract {contract_name}: {:?}",
                    suite_result.test_results.keys()
                );

                let test_result = &actuals[*contract_name].test_results[*test_name];
                assert_eq!(test_result.status, TestStatus::Success);

                let expected: HashMap<&'static str, Option<&'static str>> = HashMap::from([(
                    "keyExists(string,string)",
                    Some("replaced by `keyExistsJson`"),
                )]);
                assert_eq!(test_result.deprecated_cheatcodes, expected);
            }
        }
    }

    let filter = SolidityTestFilter::new(".*", ".*", "default/core/DeprecatedCheatcode.t.sol");
    let runner = TEST_DATA_DEFAULT.runner().await;
    let results = runner.test_collect(filter).await.suite_results;

    assert_multiple_deprecation_warnings(
        &results,
        BTreeMap::from([
            (
                "default/core/DeprecatedCheatcode.t.sol:DeprecatedCheatcodeTest",
                vec!["test_deprecated_cheatcode()"],
            ),
            (
                "default/core/DeprecatedCheatcode.t.sol:DeprecatedCheatcodeFuzzTest",
                vec!["test_deprecated_cheatcode(uint256)"],
            ),
            (
                "default/core/DeprecatedCheatcode.t.sol:DeprecatedCheatcodeInvariantTest",
                vec!["invariant_deprecated_cheatcode()"],
            ),
        ]),
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn test_gas_report_revert() {
    let filter = SolidityTestFilter::new(
        ".*",
        ".*",
        "default/core/(FailingSetup|PaymentFailure).t.sol",
    );
    let mut config = TEST_DATA_DEFAULT.config_with_mock_rpc();
    config.generate_gas_report = true;
    let runner = TEST_DATA_DEFAULT.runner_with_fuzz_persistence(config).await;
    let SolidityTestsRunResult { test_result, .. } = runner.test_collect(filter).await;

    let gas_report = test_result.gas_report.unwrap();

    // Test reverts
    let setup_failure_reports = gas_report
        .contracts
        .get("default/core/FailingSetup.t.sol:FailingSetupTest")
        .unwrap();

    let deployment_failure = setup_failure_reports.deployments.first().unwrap();
    assert_eq!(deployment_failure.status, GasReportExecutionStatus::Revert);

    let payment_failure_reports = gas_report
        .contracts
        .get("default/core/PaymentFailure.t.sol:Payable")
        .unwrap();

    let pay_failure = payment_failure_reports
        .functions
        .get("pay()")
        .unwrap()
        .first();
    assert!(pay_failure.is_some());
    assert_eq!(pay_failure.unwrap().gas, 0);
    assert_eq!(
        pay_failure.unwrap().status,
        GasReportExecutionStatus::Revert
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn test_function_override_allow_internal_expect_revert() {
    let filter = SolidityTestFilter::new(".*", ".*", "default/core/InternalRevert.t.sol");
    let mut config = TEST_DATA_DEFAULT.config_with_mock_rpc();
    config.cheats_config_options.allow_internal_expect_revert = true;

    let runner = TEST_DATA_DEFAULT.runner_with_fuzz_persistence(config).await;
    let results = runner.test_collect(filter).await.suite_results;

    assert_multiple(
        &results,
        BTreeMap::from([(
            "default/core/InternalRevert.t.sol:InternalRevertingTest",
            vec![("testInternalRevert()", true, None, None, None)],
        )]),
    );
}
