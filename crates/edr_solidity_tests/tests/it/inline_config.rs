//! Inline-config (`forge-config:`/`hardhat-config:`) end-to-end behavior.

use std::io::Write as _;

use edr_solidity_tests::{
    inline_config::error::{InlineConfigDirectiveError, InlineConfigProblem},
    SolidityTestRunnerConfigError,
};

use crate::helpers::TEST_DATA_DEFAULT;

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

/// Ill-formed inline configuration fails runner creation — aborting the whole
/// run before any test executes (matching Hardhat/Foundry) — reporting the
/// first problem of every affected function, located at its source line.
#[tokio::test(flavor = "multi_thread")]
async fn malformed_inline_config_aborts_whole_run() {
    let mut file = tempfile::Builder::new()
        .suffix(".sol")
        .tempfile()
        .expect("temp file");
    file.write_all(MALFORMED_SOURCE.as_bytes())
        .expect("write source");

    // Point one of the test sources at the malformed file on disk; collection
    // parses it under that source's name.
    let mut config = TEST_DATA_DEFAULT.config_with_mock_rpc();
    let source = config
        .test_source_paths
        .keys()
        .next()
        .cloned()
        .expect("test data has test sources");
    config
        .test_source_paths
        .insert(source.clone(), file.path().to_path_buf());

    let error = TEST_DATA_DEFAULT
        .try_build_runner(config)
        .await
        .expect_err("runner creation fails on malformed inline config");

    let SolidityTestRunnerConfigError::InlineConfig(errors) = error else {
        panic!("expected an inline-config error, got: {error}");
    };

    // One problem per affected function, each locating its source, contract,
    // function and the line of the offending directive.
    let items = errors.items();
    assert_eq!(items.len(), 2, "{items:#?}");

    let fuzz = items
        .iter()
        .find(|item| {
            matches!(
                &item.problem,
                InlineConfigProblem::Directive(InlineConfigDirectiveError { function, .. }) if function == "testFuzzBad"
            )
        })
        .expect("testFuzzBad reported");
    assert_eq!(fuzz.source_path, source);
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
                InlineConfigProblem::Directive(InlineConfigDirectiveError { function, .. }) if function == "testOtherBad"
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
