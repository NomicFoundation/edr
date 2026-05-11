//! Tests for coverage instrumentation returndata preservation.

use std::collections::BTreeMap;

use crate::helpers::{assert_multiple, SolidityTestFilter, TEST_DATA_DEFAULT};

#[tokio::test(flavor = "multi_thread")]
async fn test_coverage_returndata() {
    let filter = SolidityTestFilter::new(".*", ".*", "default/coverage/CoverageReturndata.t.sol");
    let mut config = TEST_DATA_DEFAULT.config_with_mock_rpc();
    config.coverage = true;
    config.on_collected_coverage_fn = Some(Box::new(|_hits| Ok(())));
    let runner = TEST_DATA_DEFAULT.runner_with_fuzz_persistence(config).await;
    let results = runner.test_collect(filter).await.suite_results;

    assert_multiple(
        &results,
        BTreeMap::from([(
            "default/coverage/CoverageReturndata.t.sol:CoverageReturndataTest",
            vec![
                ("testForwardSuccessfulCall()", true, None, None, None),
                ("testForwardRevertedCall()", true, None, None, None),
                ("testDeployChild()", true, None, None, None),
                ("testDeployRevertingChild()", true, None, None, None),
            ],
        )]),
    );
}

/// Regression test for <https://github.com/NomicFoundation/edr/issues/1391> —
/// coverage instrumentation probes must not consume `vm.prank` state.
#[tokio::test(flavor = "multi_thread")]
async fn test_coverage_prank() {
    let filter = SolidityTestFilter::new(".*", ".*", "default/coverage/CoveragePrank.t.sol");
    let mut config = TEST_DATA_DEFAULT.config_with_mock_rpc();
    config.coverage = true;
    config.on_collected_coverage_fn = Some(Box::new(|_hits| Ok(())));
    let runner = TEST_DATA_DEFAULT.runner_with_fuzz_persistence(config).await;
    let results = runner.test_collect(filter).await.suite_results;

    assert_multiple(
        &results,
        BTreeMap::from([(
            "default/coverage/CoveragePrank.t.sol:CoveragePrankTest",
            vec![
                ("testPrankSurvivesCoverageProbe()", true, None, None, None),
                (
                    "testPrankWithOriginSurvivesCoverageProbe()",
                    true,
                    None,
                    None,
                    None,
                ),
                ("testStartPrankStillWorks()", true, None, None, None),
            ],
        )]),
    );
}
