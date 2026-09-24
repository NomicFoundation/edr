//! Inline-config (`forge-config:`/`hardhat-config:`) end-to-end behavior.

use std::{io::Write as _, path::PathBuf};

use edr_solidity_tests::{
    error::TestRunnerError,
    result::TestKind,
    test_source_error::{InlineConfigDirectiveError, TestSourceCollectError, TestSourceProblem},
};

use crate::helpers::{SolidityTestFilter, TEST_DATA_DEFAULT};

/// Runs every suite matching `filter` and returns the inline-config problems
/// the run was rejected with.
///
/// Collection happens when a run starts, over the suites it selected, so these
/// problems surface from the run rather than from runner creation — still
/// before any test executes.
async fn expect_inline_config_errors(
    config: edr_solidity_tests::SolidityTestRunnerConfig<edr_chain_l1::EvmHardfork>,
    filter: SolidityTestFilter,
) -> edr_solidity_tests::test_source_error::TestSourceErrors {
    let runner = TEST_DATA_DEFAULT.runner_with_config(config).await;
    let result = runner.test(
        tokio::runtime::Handle::current(),
        std::sync::Arc::new(filter),
        std::sync::Arc::new(|_| {}),
    );

    match result {
        Err(TestRunnerError::TestSources(errors)) => errors,
        Err(error) => panic!("expected an inline-config error, got: {error}"),
        Ok(_) => panic!("the run should have been rejected"),
    }
}

/// A source whose two test functions each carry a distinct malformed directive.
const MALFORMED_SOURCE: &str = r#"// SPDX-License-Identifier: MIT
pragma solidity ^0.8.0;

contract BadInlineConfig {
    /// forge-config: default.fuzz.runs = -1
    function testFuzzBad(uint256 x) public {}

    /// forge-config: fuzz.bogus = 1
    function testOtherBad() public {}
}
"#;

/// The solc source name of the test source these tests redirect or remove to
/// provoke a collection problem.
///
/// It must be one Slang parses, because collection uses the grammar of the
/// version its artifact was compiled with. Standing in for e.g. the 0.5.17
/// `FuzzPreBytecodeHash.t.sol` would report an unsupported-version problem
/// instead of the one under test.
const STAND_IN_SOURCE: &str = "default/fuzz/Fuzz.t.sol";

/// The runner's fuzz runs with no inline configuration applied. The
/// contract-level directive in `ContractLevelConfig.t.sol` sets 15, so this
/// value appearing means the directive was never collected.
const DEFAULT_FUZZ_RUNS: usize = 256;

/// Writes [`MALFORMED_SOURCE`] to a temporary `.sol` file on disk.
fn malformed_source_file() -> tempfile::NamedTempFile {
    let mut file = tempfile::Builder::new()
        .suffix(".sol")
        .tempfile()
        .expect("temp file");
    file.write_all(MALFORMED_SOURCE.as_bytes())
        .expect("write source");

    file
}

/// Resolves [`STAND_IN_SOURCE`] to the solc source name `config` knows it by.
fn stand_in_source_name(
    config: &edr_solidity_tests::SolidityTestRunnerConfig<edr_chain_l1::EvmHardfork>,
) -> PathBuf {
    config
        .test_source_paths
        .keys()
        .find(|source| source.ends_with(STAND_IN_SOURCE))
        .cloned()
        .expect("test data contains the stand-in source")
}

/// The filter selecting only [`STAND_IN_SOURCE`]'s suite.
fn stand_in_filter() -> SolidityTestFilter {
    SolidityTestFilter::new(".*", ".*", &format!(".*{STAND_IN_SOURCE}"))
}

/// The solc source name of the test source compiled with a version that
/// predates the oldest Solidity grammar, used to provoke that problem.
const PRE_0_8_SOURCE: &str = "default/fuzz/FuzzPreBytecodeHash.t.sol";

/// The sole test contract [`PRE_0_8_SOURCE`] declares.
const PRE_0_8_CONTRACT: &str = "FuzzPreBytecodeHash";

/// The filter selecting only [`PRE_0_8_SOURCE`]'s suite.
fn pre_0_8_filter() -> SolidityTestFilter {
    SolidityTestFilter::new(".*", ".*", &format!(".*{PRE_0_8_SOURCE}"))
}

/// Ill-formed inline configuration aborts the whole run when it starts, before
/// any test executes (matching Hardhat/Foundry), reporting the first problem of
/// every affected function, located at its source line.
#[tokio::test(flavor = "multi_thread")]
async fn malformed_inline_config_aborts_whole_run() {
    let file = malformed_source_file();

    // Point the stand-in source at the malformed file on disk, because
    // collection parses it under that source's name.
    let mut config = TEST_DATA_DEFAULT.config_with_mock_rpc();
    let source = stand_in_source_name(&config);
    config
        .test_source_paths
        .insert(source.clone(), file.path().to_path_buf());

    let errors = expect_inline_config_errors(config, stand_in_filter()).await;

    // One problem per affected function, each locating its source, contract,
    // function and the line of the offending directive.
    let items = errors.items();
    assert_eq!(items.len(), 2, "{items:#?}");

    let fuzz = items
        .iter()
        .find(|item| {
            matches!(
                &item.problem,
                TestSourceProblem::Directive(InlineConfigDirectiveError { function, .. })
                    if function.as_deref() == Some("testFuzzBad")
            )
        })
        .expect("testFuzzBad reported");
    assert_eq!(fuzz.source_name, source);
    let TestSourceProblem::Directive(InlineConfigDirectiveError { contract, line, .. }) =
        &fuzz.problem
    else {
        unreachable!("filtered to a testFuzzBad directive above");
    };
    assert_eq!(contract, "BadInlineConfig");
    assert_eq!(*line, 5);

    let other = items
        .iter()
        .find(|item| {
            matches!(
                &item.problem,
                TestSourceProblem::Directive(InlineConfigDirectiveError { function, .. })
                    if function.as_deref() == Some("testOtherBad")
            )
        })
        .expect("testOtherBad reported");
    let TestSourceProblem::Directive(InlineConfigDirectiveError { line, .. }) = &other.problem
    else {
        unreachable!("filtered to a testOtherBad directive above");
    };
    assert_eq!(*line, 8);

    // The rendered report names the source and both functions.
    let rendered = errors.to_string();
    assert!(
        rendered.contains(&source.display().to_string()),
        "{rendered}"
    );
    assert!(
        rendered.contains("BadInlineConfig.testFuzzBad"),
        "{rendered}"
    );
    assert!(
        rendered.contains("BadInlineConfig.testOtherBad"),
        "{rendered}"
    );
}

/// A directive on a test-named function that matches nothing in the contract
/// ABI (e.g. not externally callable) cannot take effect: the function never
/// runs as a test. The suite reports a warning instead of silently ignoring
/// the directive.
#[tokio::test(flavor = "multi_thread")]
async fn unmatched_function_directive_warns() {
    let filter = SolidityTestFilter::new(".*", ".*", ".*inline/UnmatchedInlineConfig.t.sol");
    let config = TEST_DATA_DEFAULT.config_with_mock_rpc();
    let runner = TEST_DATA_DEFAULT.runner_with_config(config).await;
    let results = runner
        .test_collect(filter)
        .await
        .expect("the run produces results")
        .suite_results;

    let suite = results
        .get("default/inline/UnmatchedInlineConfig.t.sol:UnmatchedInlineConfigTest")
        .expect("suite ran");
    assert!(
        suite.test_results.contains_key("test_Runs()"),
        "{:#?}",
        suite.test_results.keys()
    );

    assert_eq!(suite.warnings.len(), 1, "{:#?}", suite.warnings);
    let warning = &suite.warnings[0];
    assert!(
        warning.contains("testFuzz_NotExternallyCallable")
            && warning.contains("UnmatchedInlineConfigTest"),
        "{warning}"
    );
}

/// A contract-level directive (NatSpec above the contract definition) applies
/// to every test the contract runs — including inherited ones — with
/// function-level directives taking per-key precedence.
#[tokio::test(flavor = "multi_thread")]
async fn contract_level_inline_config_applies_to_all_tests() {
    let filter = SolidityTestFilter::new(".*", ".*", ".*inline/ContractLevelConfig.t.sol");
    let config = TEST_DATA_DEFAULT.config_with_mock_rpc();
    let runner = TEST_DATA_DEFAULT.runner_with_fuzz_persistence(config).await;
    let results = runner
        .test_collect(filter)
        .await
        .expect("the run produces results")
        .suite_results;

    let suite = results
        .get("default/inline/ContractLevelConfig.t.sol:ContractLevelConfigTest")
        .expect("suite ran");

    let fuzz_runs = |test_name: &str| -> u32 {
        let result = suite
            .test_results
            .get(test_name)
            .unwrap_or_else(|| panic!("{test_name} ran"));
        match result.kind {
            TestKind::Fuzz { runs, .. } => u32::try_from(runs).expect("runs fit in u32"),
            ref kind => panic!("{test_name} is a fuzz test, got {kind:?}"),
        }
    };

    // The contract-level `fuzz.runs = 15` covers functions with no directive of
    // their own, whether declared directly or inherited from a base contract.
    assert_eq!(fuzz_runs("testFuzz_ContractLevelRuns(uint256)"), 15);
    assert_eq!(fuzz_runs("testFuzz_InheritedRuns(uint256)"), 15);
    // A function-level directive wins over the contract level.
    assert_eq!(fuzz_runs("testFuzz_FunctionOverridesContract(uint256)"), 20);

    // Overloaded test functions are distinct tests; each overload gets the
    // contract-level configuration.
    assert_eq!(fuzz_runs("testFuzz_Overloaded(uint256)"), 15);
    assert_eq!(fuzz_runs("testFuzz_Overloaded(uint256,uint256)"), 15);
    // A function-level directive identifies its function by name only, so it
    // applies to every overload of that name.
    assert_eq!(fuzz_runs("testFuzz_OverloadedWithDirective(uint256)"), 25);
    assert_eq!(
        fuzz_runs("testFuzz_OverloadedWithDirective(uint256,uint256)"),
        25
    );

    // The contract-level invariant section applies to the invariant test.
    let invariant = suite
        .test_results
        .get("invariant_ContractLevelRuns()")
        .expect("invariant test ran");
    assert!(
        matches!(
            invariant.kind,
            TestKind::Invariant {
                runs: 2,
                calls: 6,
                ..
            }
        ),
        "expected 2 runs of depth 3 (6 calls), got {:?}",
        invariant.kind
    );
}

/// A test source missing from `test_source_paths` is never located or read, so
/// its inline configuration and EIP-712 types would silently go uncollected.
/// The run is rejected before any test executes instead.
#[tokio::test(flavor = "multi_thread")]
async fn source_without_a_path_aborts_whole_run() {
    let file = malformed_source_file();

    // A missing entry is found while locating roots, a malformed directive
    // while parsing one. The two are collected separately and merged, so a run
    // hitting both must report both.
    let mut config = TEST_DATA_DEFAULT.config_with_mock_rpc();
    let unlisted = stand_in_source_name(&config);
    config.test_source_paths.remove(&unlisted);

    let malformed = config
        .test_source_paths
        .keys()
        .find(|source| source.ends_with("inline/ContractLevelConfig.t.sol"))
        .cloned()
        .expect("test data contains the contract-level config source");
    config
        .test_source_paths
        .insert(malformed.clone(), file.path().to_path_buf());

    let errors = expect_inline_config_errors(
        config,
        SolidityTestFilter::new(
            ".*",
            ".*",
            ".*(fuzz/Fuzz|inline/ContractLevelConfig)\\.t\\.sol",
        ),
    )
    .await;

    let items = errors.items();
    assert!(
        items.iter().any(|item| item.source_name == unlisted
            && matches!(
                &item.problem,
                TestSourceProblem::Source(TestSourceCollectError::SourcePathNotProvided)
            )),
        "{items:#?}"
    );
    assert!(
        items.iter().any(|item| item.source_name == malformed
            && matches!(&item.problem, TestSourceProblem::Directive(_))),
        "{items:#?}"
    );
}

/// Disabling collection stops inline configuration taking effect, not just
/// EIP-712 resolution. The contract-level `fuzz.runs` is ignored, so the
/// runner's own default applies.
#[tokio::test(flavor = "multi_thread")]
async fn collection_disabled_ignores_inline_config() {
    let mut config = TEST_DATA_DEFAULT.config_with_mock_rpc();
    config.test_source_paths.clear();

    let runner = TEST_DATA_DEFAULT.runner_with_fuzz_persistence(config).await;
    let results = runner
        .test_collect(SolidityTestFilter::new(
            ".*",
            ".*",
            ".*inline/ContractLevelConfig.t.sol",
        ))
        .await
        .expect("collection is disabled, so nothing can reject the run")
        .suite_results;

    let suite = results
        .get("default/inline/ContractLevelConfig.t.sol:ContractLevelConfigTest")
        .expect("suite ran");
    let result = suite
        .test_results
        .get("testFuzz_ContractLevelRuns(uint256)")
        .expect("the test ran");

    match result.kind {
        TestKind::Fuzz { runs, .. } => assert_eq!(runs, DEFAULT_FUZZ_RUNS),
        ref kind => panic!("expected a fuzz test, got {kind:?}"),
    }
}

/// Collection requires solc 0.8 or newer, so a run that selects a pre-0.8
/// source and provides a source-path map is rejected, naming the version.
#[tokio::test(flavor = "multi_thread")]
async fn pre_0_8_source_aborts_whole_run() {
    let errors =
        expect_inline_config_errors(TEST_DATA_DEFAULT.config_with_mock_rpc(), pre_0_8_filter())
            .await;

    let items = errors.items();
    assert_eq!(items.len(), 1, "{items:#?}");
    assert!(
        items[0].source_name.ends_with(PRE_0_8_SOURCE),
        "{:#?}",
        items[0].source_name
    );
    assert!(
        matches!(
            &items[0].problem,
            TestSourceProblem::Source(TestSourceCollectError::UnsupportedSolcVersion {
                version
            }) if *version == semver::Version::new(0, 5, 17)
        ),
        "{:#?}",
        items[0].problem
    );
}

/// Omitting the source-path map disables collection entirely, which is how a
/// project whose test sources predate solc 0.8 keeps running its tests. The
/// suite runs, and simply gets no inline configuration and no EIP-712 types.
#[tokio::test(flavor = "multi_thread")]
async fn pre_0_8_source_runs_when_collection_is_disabled() {
    let mut config = TEST_DATA_DEFAULT.config_with_mock_rpc();
    config.test_source_paths.clear();

    let runner = TEST_DATA_DEFAULT.runner_with_config(config).await;
    let results = runner
        .test_collect(pre_0_8_filter())
        .await
        .expect("collection is disabled, so nothing can reject the run")
        .suite_results;

    let suite = results
        .get(&format!("{PRE_0_8_SOURCE}:{PRE_0_8_CONTRACT}"))
        .expect("the suite runs");
    assert!(
        !suite.test_results.is_empty(),
        "the suite's tests should have executed"
    );
    assert!(suite.warnings.is_empty(), "{:#?}", suite.warnings);
}

/// Only the sources of the suites a run selects are parsed. A filter that
/// excludes a broken source must not pay for parsing it — nor be failed by it.
#[tokio::test(flavor = "multi_thread")]
async fn filtered_out_sources_are_not_parsed() {
    let file = malformed_source_file();

    // Point the stand-in source at the malformed file, then filter to a
    // different one.
    let mut config = TEST_DATA_DEFAULT.config_with_mock_rpc();
    let source = stand_in_source_name(&config);
    config
        .test_source_paths
        .insert(source, file.path().to_path_buf());

    let filter = SolidityTestFilter::new(".*", ".*", ".*inline/ContractLevelConfig.t.sol");
    let runner = TEST_DATA_DEFAULT.runner_with_fuzz_persistence(config).await;
    let results = runner
        .test_collect(filter)
        .await
        .expect("the run produces results")
        .suite_results;

    // The malformed source belongs to a suite this run did not select, so it
    // was never parsed and its problems never surfaced.
    assert!(
        results.contains_key("default/inline/ContractLevelConfig.t.sol:ContractLevelConfigTest"),
        "{:#?}",
        results.keys()
    );
}
