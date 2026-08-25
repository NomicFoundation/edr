//! Inline-config (`forge-config:`/`hardhat-config:`) end-to-end behavior.

use std::io::Write as _;

use edr_solidity_tests::{
    inline_config::InlineConfigProblem, result::TestKind, SolidityTestRunnerConfigError,
};

use crate::helpers::{SolidityTestFilter, TEST_DATA_DEFAULT};

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
    // parses it under that source's name. The source must be picked
    // deterministically and be compiled with a solc version Slang supports:
    // collection parses each source with the grammar of the version its
    // artifact was compiled with, so redirecting e.g. the 0.5.17
    // `FuzzPreBytecodeHash.t.sol` would report a source-level
    // `InvalidSolcVersion` error instead of the directive errors.
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
                InlineConfigProblem::Directive { function, .. } if function.as_deref() == Some("testFuzzBad")
            )
        })
        .expect("testFuzzBad reported");
    assert_eq!(fuzz.source, source);
    let InlineConfigProblem::Directive { contract, line, .. } = &fuzz.problem else {
        unreachable!("filtered to a testFuzzBad directive above");
    };
    assert_eq!(contract, "BadInlineConfig");
    assert_eq!(*line, 5);

    let other = items
        .iter()
        .find(|item| {
            matches!(
                &item.problem,
                InlineConfigProblem::Directive { function, .. } if function.as_deref() == Some("testOtherBad")
            )
        })
        .expect("testOtherBad reported");
    let InlineConfigProblem::Directive { line, .. } = &other.problem else {
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

/// A contract-level directive (NatSpec above the contract definition) applies
/// to every test the contract runs — including inherited ones — with
/// function-level directives taking per-key precedence.
#[tokio::test(flavor = "multi_thread")]
async fn contract_level_inline_config_applies_to_all_tests() {
    let filter = SolidityTestFilter::new(".*", ".*", ".*inline/ContractLevelConfig.t.sol");
    let config = TEST_DATA_DEFAULT.config_with_mock_rpc();
    let runner = TEST_DATA_DEFAULT.runner_with_fuzz_persistence(config).await;
    let results = runner.test_collect(filter).await.suite_results;

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
        "expected 2 runs of depth 3, got {:?}",
        invariant.kind
    );
}
