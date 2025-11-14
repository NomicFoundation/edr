//! Table tests.

use std::collections::BTreeMap;

use crate::helpers::{assert_multiple, SolidityTestFilter, TEST_DATA_DEFAULT};

#[tokio::test(flavor = "multi_thread")]
async fn test_table() {
    let filter = SolidityTestFilter::new(".*", ".*", ".*table/");
    let runner = TEST_DATA_DEFAULT.runner().await;
    let results = runner.test_collect(filter).await.suite_results;

    assert_multiple(
        &results,
        BTreeMap::from([(
            "default/table/CounterTable.t.sol:CounterTableTest",
            vec![
                (
                    "tableMultipleParamsDifferentFixturesFail(uint256,bool)",
                    false,
                    Some("2 fixtures defined for diffSwap (expected 10)".into()),
                    None,
                    None,
                ),
                (
                    "tableMultipleParamsFail(uint256,bool)",
                    false,
                    Some("Cannot swap".into()),
                    None,
                    None,
                ),
                (
                    "tableMultipleParamsNoParamFail(uint256,bool)",
                    false,
                    Some("No fixture defined for param noSwap".into()),
                    None,
                    None,
                ),
                (
                    "tableMultipleParamsPass(uint256,bool)",
                    true,
                    None,
                    None,
                    None,
                ),
                (
                    "tableSingleParamFail(uint256)",
                    false,
                    Some("Amount cannot be 10".into()),
                    None,
                    None,
                ),
                ("tableSingleParamPass(uint256)", true, None, None, None),
                (
                    "tableWithNoParamFail()",
                    false,
                    Some("Table test should have at least one parameter".into()),
                    None,
                    None,
                ),
                (
                    "tableWithParamNoFixtureFail(uint256)",
                    false,
                    Some("Table test should have at least one fixture".into()),
                    None,
                    None,
                ),
            ],
        )]),
    );
}
