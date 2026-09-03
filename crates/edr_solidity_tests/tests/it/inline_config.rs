//! Inline-config (`forge-config:`/`hardhat-config:`) end-to-end behavior.

use std::io::Write as _;

use edr_solidity_tests::{
    error::TestRunnerError,
    inline_config::error::{
        InlineConfigCollectError, InlineConfigDirectiveError, InlineConfigProblem,
    },
    result::TestKind,
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
) -> edr_solidity_tests::inline_config::error::InlineConfigErrors {
    let runner = TEST_DATA_DEFAULT.runner_with_config(config).await;
    let result = runner.test(
        tokio::runtime::Handle::current(),
        std::sync::Arc::new(filter),
        std::sync::Arc::new(|_| {}),
    );

    match result {
        Err(TestRunnerError::InlineConfig(errors)) => errors,
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

/// Ill-formed inline configuration aborts the whole run when it starts, before
/// any test executes (matching Hardhat/Foundry), reporting the first problem of
/// every affected function, located at its source line.
#[tokio::test(flavor = "multi_thread")]
async fn malformed_inline_config_aborts_whole_run() {
    let mut file = tempfile::Builder::new()
        .suffix(".sol")
        .tempfile()
        .expect("temp file");
    file.write_all(MALFORMED_SOURCE.as_bytes())
        .expect("write source");

    // Point one of the test sources at the malformed file on disk; collection
    // parses it under that source's name. The source must be picked
    // deterministically and be compiled with a solc version Slang supports:
    // collection parses each source with the grammar of the version its
    // artifact was compiled with, so redirecting e.g. the 0.5.17
    // `FuzzPreBytecodeHash.t.sol` would report a source-level
    // would be skipped with an unsupported-version warning, so no directive
    // errors would be reported at all.
    let mut config = TEST_DATA_DEFAULT.config_with_mock_rpc();
    let source = config
        .test_source_paths
        .keys()
        .find(|source| source.ends_with("default/fuzz/Fuzz.t.sol"))
        .cloned()
        .expect("test data contains the fuzz test source");
    config
        .test_source_paths
        .insert(source.clone(), file.path().to_path_buf());

    let errors = expect_inline_config_errors(
        config,
        SolidityTestFilter::new(".*", ".*", ".*fuzz/Fuzz.t.sol"),
    )
    .await;

    // One problem per affected function, each locating its source, contract,
    // function and the line of the offending directive.
    let items = errors.items();
    assert_eq!(items.len(), 2, "{items:#?}");

    let fuzz = items
        .iter()
        .find(|item| {
            matches!(
                &item.problem,
                InlineConfigProblem::Directive(InlineConfigDirectiveError { function, .. })
                    if function.as_deref() == Some("testFuzzBad")
            )
        })
        .expect("testFuzzBad reported");
    assert_eq!(fuzz.source_name, source);
    let InlineConfigProblem::Directive(InlineConfigDirectiveError { contract, line, .. }) =
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
                InlineConfigProblem::Directive(InlineConfigDirectiveError { function, .. })
                    if function.as_deref() == Some("testOtherBad")
            )
        })
        .expect("testOtherBad reported");
    let InlineConfigProblem::Directive(InlineConfigDirectiveError { line, .. }) = &other.problem
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
    let mut config = TEST_DATA_DEFAULT.config_with_mock_rpc();
    let source = config
        .test_source_paths
        .keys()
        .find(|source| source.ends_with("default/fuzz/Fuzz.t.sol"))
        .cloned()
        .expect("test data contains the fuzz test source");
    config.test_source_paths.remove(&source);

    let errors = expect_inline_config_errors(
        config,
        SolidityTestFilter::new(".*", ".*", ".*fuzz/Fuzz.t.sol"),
    )
    .await;

    let items = errors.items();
    assert_eq!(items.len(), 1, "{items:#?}");
    assert_eq!(items[0].source_name, source);
    assert!(
        matches!(
            &items[0].problem,
            InlineConfigProblem::Source(InlineConfigCollectError::SourcePathNotProvided)
        ),
        "{:#?}",
        items[0].problem
    );
}

/// A test source Slang has no grammar for — solc older than 0.8 — is skipped
/// rather than failing the run: it uses neither inline configuration nor the
/// EIP-712 cheatcodes. The suite still runs, and says why nothing was
/// collected from it.
#[tokio::test(flavor = "multi_thread")]
async fn pre_0_8_source_is_skipped_with_a_warning() {
    let filter = SolidityTestFilter::new(".*", ".*", ".*fuzz/FuzzPreBytecodeHash.t.sol");
    let config = TEST_DATA_DEFAULT.config_with_mock_rpc();
    let runner = TEST_DATA_DEFAULT.runner_with_config(config).await;
    let results = runner
        .test_collect(filter)
        .await
        .expect("the run produces results")
        .suite_results;

    let suite = results
        .get("default/fuzz/FuzzPreBytecodeHash.t.sol:FuzzPreBytecodeHash")
        .expect("the suite still runs");
    assert!(
        !suite.test_results.is_empty(),
        "the suite's tests should have executed"
    );

    assert_eq!(suite.warnings.len(), 1, "{:#?}", suite.warnings);
    let warning = &suite.warnings[0];
    assert!(
        warning.contains("FuzzPreBytecodeHash.t.sol") && warning.contains("EIP-712"),
        "{warning}"
    );
}

/// Only the sources of the suites a run selects are parsed. A filter that
/// excludes a broken source must not pay for parsing it — nor be failed by it.
#[tokio::test(flavor = "multi_thread")]
async fn filtered_out_sources_are_not_parsed() {
    let mut file = tempfile::Builder::new()
        .suffix(".sol")
        .tempfile()
        .expect("temp file");
    file.write_all(MALFORMED_SOURCE.as_bytes())
        .expect("write source");

    // Point one source at the malformed file, then filter to a different one.
    let mut config = TEST_DATA_DEFAULT.config_with_mock_rpc();
    let source = config
        .test_source_paths
        .keys()
        .find(|source| source.ends_with("default/fuzz/Fuzz.t.sol"))
        .cloned()
        .expect("test data contains the fuzz test source");
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
