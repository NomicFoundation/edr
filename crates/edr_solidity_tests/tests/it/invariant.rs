//! Invariant tests.

use std::collections::BTreeMap;

use alloy_primitives::U256;
use edr_solidity_tests::fuzz::CounterExample;

use crate::helpers::{
    assert_multiple, SolidityTestFilter, TestFuzzConfig, TestInvariantConfig, TEST_DATA_DEFAULT,
};

macro_rules! get_counterexample {
    ($runner:ident, $filter:expr) => {
        $runner
            .test_collect($filter)
            .await
            .values()
            .last()
            .expect("Invariant contract should be testable.")
            .test_results
            .values()
            .last()
            .expect("Invariant contract should be testable.")
            .counterexample
            .as_ref()
            .expect("Invariant contract should have failed with a counterexample.")
    };
}

#[tokio::test(flavor = "multi_thread")]
async fn test_invariant() {
    let failure_persist_dir = tempfile::tempdir().expect("Can create temp dir");
    let filter = SolidityTestFilter::new(".*", ".*", ".*fuzz/invariant/(target|targetAbi|common)");
    let runner = TEST_DATA_DEFAULT
        .runner_with_invariant_config(TestInvariantConfig {
            failure_persist_dir: Some(failure_persist_dir.path().into()),
            ..TestInvariantConfig::default()
        })
        .await;

    let results = runner.test_collect(filter).await;

    assert_multiple(
        &results,
        BTreeMap::from([
            (
                "default/fuzz/invariant/common/InvariantHandlerFailure.t.sol:InvariantHandlerFailure",
                vec![("statefulFuzz_BrokenInvariant()", true, None, None, None)],
            ),
            (
                "default/fuzz/invariant/common/InvariantInnerContract.t.sol:InvariantInnerContract",
                vec![(
                    "invariantHideJesus()",
                    false,
                    Some("revert: jesus betrayed".into()),
                    None,
                    None,
                )],
            ),
            (
                "default/fuzz/invariant/common/InvariantReentrancy.t.sol:InvariantReentrancy",
                vec![("invariantNotStolen()", true, None, None, None)],
            ),
            (
                "default/fuzz/invariant/common/InvariantTest1.t.sol:InvariantTest",
                vec![
                    ("invariant_neverFalse()", false, Some("revert: false".into()), None, None),
                    (
                        "statefulFuzz_neverFalseWithInvariantAlias()",
                        false,
                        Some("revert: false".into()),
                        None,
                        None,
                    ),
                ],
            ),
            (
                "default/fuzz/invariant/target/ExcludeContracts.t.sol:ExcludeContracts",
                vec![("invariantTrueWorld()", true, None, None, None)],
            ),
            (
                "default/fuzz/invariant/target/TargetContracts.t.sol:TargetContracts",
                vec![("invariantTrueWorld()", true, None, None, None)],
            ),
            (
                "default/fuzz/invariant/target/TargetSenders.t.sol:TargetSenders",
                vec![(
                    "invariantTrueWorld()",
                    false,
                    Some("revert: false world".into()),
                    None,
                    None,
                )],
            ),
            (
                "default/fuzz/invariant/target/TargetInterfaces.t.sol:TargetWorldInterfaces",
                vec![(
                    "invariantTrueWorld()",
                    false,
                    Some("revert: false world".into()),
                    None,
                    None,
                )],
            ),
            (
                "default/fuzz/invariant/target/ExcludeSenders.t.sol:ExcludeSenders",
                vec![("invariantTrueWorld()", true, None, None, None)],
            ),
            (
                "default/fuzz/invariant/target/TargetSelectors.t.sol:TargetSelectors",
                vec![("invariantTrueWorld()", true, None, None, None)],
            ),
            (
                "default/fuzz/invariant/targetAbi/ExcludeArtifacts.t.sol:ExcludeArtifacts",
                vec![("invariantShouldPass()", true, None, None, None)],
            ),
            (
                "default/fuzz/invariant/targetAbi/TargetArtifacts.t.sol:TargetArtifacts",
                vec![
                    ("invariantShouldPass()", true, None, None, None),
                    (
                        "invariantShouldFail()",
                        false,
                        Some("revert: false world".into()),
                        None,
                        None,
                    ),
                ],
            ),
            (
                "default/fuzz/invariant/targetAbi/TargetArtifactSelectors.t.sol:TargetArtifactSelectors",
                vec![("invariantShouldPass()", true, None, None, None)],
            ),
            (
                "default/fuzz/invariant/targetAbi/TargetArtifactSelectors2.t.sol:TargetArtifactSelectors2",
                vec![(
                    "invariantShouldFail()",
                    false,
                    Some("revert: it's false".into()),
                    None,
                    None,
                )],
            ),
            (
                "default/fuzz/invariant/common/InvariantShrinkWithAssert.t.sol:InvariantShrinkWithAssert",
                vec![(
                    "invariant_with_assert()",
                    false,
                    Some("<empty revert data>".into()),
                    None,
                    None,
                )],
            ),
            (
                "default/fuzz/invariant/common/InvariantShrinkWithAssert.t.sol:InvariantShrinkWithRequire",
                vec![(
                    "invariant_with_require()",
                    false,
                    Some("revert: wrong counter".into()),
                    None,
                    None,
                )],
            ),
            (
                "default/fuzz/invariant/common/InvariantPreserveState.t.sol:InvariantPreserveState",
                vec![("invariant_preserve_state()", true, None, None, None)],
            ),
            (
                "default/fuzz/invariant/common/InvariantCalldataDictionary.t.sol:InvariantCalldataDictionary",
                vec![(
                    "invariant_owner_never_changes()",
                    false,
                    Some("<empty revert data>".into()),
                    None,
                    None,
                )],
            ),
            (
                "default/fuzz/invariant/common/InvariantAssume.t.sol:InvariantAssume",
                vec![("invariant_dummy()", true, None, None, None)],
            ),
            (
                "default/fuzz/invariant/common/InvariantCustomError.t.sol:InvariantCustomError",
                vec![("invariant_decode_error()", true, None, None, None)],
            ),
            (
                "default/fuzz/invariant/target/FuzzedTargetContracts.t.sol:ExplicitTargetContract",
                vec![("invariant_explicit_target()", true, None, None, None)],
            ),
            (
                "default/fuzz/invariant/target/FuzzedTargetContracts.t.sol:DynamicTargetContract",
                vec![("invariant_dynamic_targets()", true, None, None, None)],
            ),
            (
                "default/fuzz/invariant/common/InvariantFixtures.t.sol:InvariantFixtures",
                vec![(
                    "invariant_target_not_compromised()",
                    false,
                    Some("<empty revert data>".into()),
                    None,
                    None,
                )],
            ),
            (
                "default/fuzz/invariant/common/InvariantShrinkBigSequence.t.sol:ShrinkBigSequenceTest",
                vec![("invariant_shrink_big_sequence()", true, None, None, None)],
            ),
            (
                "default/fuzz/invariant/common/InvariantShrinkFailOnRevert.t.sol:ShrinkFailOnRevertTest",
                vec![("invariant_shrink_fail_on_revert()", true, None, None, None)],
            ),
            (
                "default/fuzz/invariant/common/InvariantScrapeValues.t.sol:FindFromReturnValueTest",
                vec![(
                    "invariant_value_not_found()",
                    false,
                    Some("revert: value from return found".into()),
                    None,
                    None,
                )],
            ),
            (
                "default/fuzz/invariant/common/InvariantScrapeValues.t.sol:FindFromLogValueTest",
                vec![(
                    "invariant_value_not_found()",
                    false,
                    Some("revert: value from logs found".into()),
                    None,
                    None,
                )],
            )
        ]),
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn test_invariant_override() {
    let filter = SolidityTestFilter::new(
        ".*",
        ".*",
        ".*fuzz/invariant/common/InvariantReentrancy.t.sol",
    );
    let runner = TEST_DATA_DEFAULT
        .runner_with_invariant_config(TestInvariantConfig {
            fail_on_revert: false,
            call_override: true,
            ..TestInvariantConfig::default()
        })
        .await;
    let results = runner.test_collect(filter).await;

    assert_multiple(
        &results,
        BTreeMap::from([(
            "default/fuzz/invariant/common/InvariantReentrancy.t.sol:InvariantReentrancy",
            vec![(
                "invariantNotStolen()",
                false,
                Some("revert: stolen".into()),
                None,
                None,
            )],
        )]),
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn test_invariant_fail_on_revert() {
    let filter = SolidityTestFilter::new(
        ".*",
        ".*",
        ".*fuzz/invariant/common/InvariantHandlerFailure.t.sol",
    );
    let runner = TEST_DATA_DEFAULT
        .runner_with_invariant_config(TestInvariantConfig {
            fail_on_revert: true,
            runs: 1,
            depth: 10,
            ..TestInvariantConfig::default()
        })
        .await;
    let results = runner.test_collect(filter).await;

    assert_multiple(
        &results,
        BTreeMap::from([(
            "default/fuzz/invariant/common/InvariantHandlerFailure.t.sol:InvariantHandlerFailure",
            vec![(
                "statefulFuzz_BrokenInvariant()",
                false,
                Some("revert: failed on revert".into()),
                None,
                None,
            )],
        )]),
    );
}

/// Disabled in https://github.com/foundry-rs/foundry/pull/5986
#[tokio::test(flavor = "multi_thread")]
#[ignore]
async fn test_invariant_storage() {
    let filter = SolidityTestFilter::new(
        ".*",
        ".*",
        ".*fuzz/invariant/storage/InvariantStorageTest.t.sol",
    );
    let runner = TEST_DATA_DEFAULT
        .runner_with_invariant_config_and_seed(
            U256::from(6u32),
            TestInvariantConfig {
                depth: 100 + (50 * u32::from(cfg!(windows))),
                ..TestInvariantConfig::default()
            },
        )
        .await;
    let results = runner.test_collect(filter).await;

    assert_multiple(
        &results,
        BTreeMap::from([(
            "default/fuzz/invariant/storage/InvariantStorageTest.t.sol:InvariantStorageTest",
            vec![
                (
                    "invariantChangeAddress()",
                    false,
                    Some("changedAddr".to_string()),
                    None,
                    None,
                ),
                (
                    "invariantChangeString()",
                    false,
                    Some("changedString".to_string()),
                    None,
                    None,
                ),
                (
                    "invariantChangeUint()",
                    false,
                    Some("changedUint".to_string()),
                    None,
                    None,
                ),
                (
                    "invariantPush()",
                    false,
                    Some("pushUint".to_string()),
                    None,
                    None,
                ),
            ],
        )]),
    );
}

#[tokio::test(flavor = "multi_thread")]
#[cfg_attr(windows, ignore = "for some reason there's different rng")]
async fn test_invariant_shrink() {
    let filter = SolidityTestFilter::new(
        ".*",
        ".*",
        ".*fuzz/invariant/common/InvariantInnerContract.t.sol",
    );
    let runner = TEST_DATA_DEFAULT
        .runner_with_fuzz_config(TestFuzzConfig {
            seed: Some(U256::from(119u32)),
            ..TestFuzzConfig::default()
        })
        .await;

    match get_counterexample!(runner, filter) {
        CounterExample::Single(_) => panic!("CounterExample should be a sequence."),
        // `fuzz_seed` at 119 makes this sequence shrinkable from 4 to 2.
        CounterExample::Sequence(sequence) => {
            assert!(sequence.len() <= 3);

            if sequence.len() == 2 {
                // call order should always be preserved
                let create_fren_sequence = sequence[0].clone();
                assert_eq!(
                    create_fren_sequence.contract_name.unwrap(),
                    "default/fuzz/invariant/common/InvariantInnerContract.t.sol:Jesus"
                );
                assert_eq!(create_fren_sequence.signature.unwrap(), "create_fren()");

                let betray_sequence = sequence[1].clone();
                assert_eq!(
                    betray_sequence.contract_name.unwrap(),
                    "default/fuzz/invariant/common/InvariantInnerContract.t.sol:Judas"
                );
                assert_eq!(betray_sequence.signature.unwrap(), "betray()");
            }
        }
    };
}

#[tokio::test(flavor = "multi_thread")]
#[cfg_attr(windows, ignore = "for some reason there's different rng")]
async fn test_invariant_assert_shrink() {
    let fuzz_config = TestFuzzConfig {
        seed: Some(U256::from(119u32)),
        ..TestFuzzConfig::default()
    };

    // ensure assert and require shrinks to same sequence of 3 or less
    test_shrink(fuzz_config.clone(), "InvariantShrinkWithAssert").await;
    test_shrink(fuzz_config, "InvariantShrinkWithRequire").await;
}

async fn test_shrink(fuzz_config: TestFuzzConfig, contract_pattern: &str) {
    let filter = SolidityTestFilter::new(
        ".*",
        contract_pattern,
        ".*fuzz/invariant/common/InvariantShrinkWithAssert.t.sol",
    );
    let runner = TEST_DATA_DEFAULT.runner_with_fuzz_config(fuzz_config).await;

    match get_counterexample!(runner, filter) {
        CounterExample::Single(_) => panic!("CounterExample should be a sequence."),
        CounterExample::Sequence(sequence) => {
            assert!(sequence.len() <= 3);
        }
    };
}

#[tokio::test(flavor = "multi_thread")]
#[cfg_attr(windows, ignore = "for some reason there's different rng")]
async fn test_shrink_big_sequence() {
    let filter = SolidityTestFilter::new(
        ".*",
        ".*",
        ".*fuzz/invariant/common/InvariantShrinkBigSequence.t.sol",
    );

    let runner = TEST_DATA_DEFAULT
        .runner_with_invariant_config_and_seed(
            U256::from(119u32),
            TestInvariantConfig {
                runs: 1,
                depth: 500,
                ..TestInvariantConfig::default()
            },
        )
        .await;

    let initial_counterexample = runner
        .clone()
        .test_collect(filter.clone())
        .await
        .values()
        .last()
        .expect("Invariant contract should be testable.")
        .test_results
        .values()
        .last()
        .expect("Invariant contract should be testable.")
        .counterexample
        .clone()
        .unwrap();

    let initial_sequence = match initial_counterexample {
        CounterExample::Single(_) => panic!("CounterExample should be a sequence."),
        CounterExample::Sequence(sequence) => sequence,
    };
    // ensure shrinks to same sequence of 77
    assert_eq!(initial_sequence.len(), 77);

    // test failure persistence
    let results = runner.test_collect(filter).await;
    assert_multiple(
        &results,
        BTreeMap::from([(
            "default/fuzz/invariant/common/InvariantShrinkBigSequence.t.sol:ShrinkBigSequenceTest",
            vec![(
                "invariant_shrink_big_sequence()",
                false,
                Some("invariant_shrink_big_sequence replay failure".into()),
                None,
                None,
            )],
        )]),
    );
    let new_sequence = match results
        .values()
        .last()
        .expect("Invariant contract should be testable.")
        .test_results
        .values()
        .last()
        .expect("Invariant contract should be testable.")
        .counterexample
        .clone()
        .unwrap()
    {
        CounterExample::Single(_) => panic!("CounterExample should be a sequence."),
        CounterExample::Sequence(sequence) => sequence,
    };
    // ensure shrinks to same sequence of 77
    assert_eq!(new_sequence.len(), 77);
    // ensure calls within failed sequence are the same as initial one
    for index in 0..77 {
        let new_call = new_sequence.get(index).unwrap();
        let initial_call = initial_sequence.get(index).unwrap();
        assert_eq!(new_call.sender, initial_call.sender);
        assert_eq!(new_call.addr, initial_call.addr);
        assert_eq!(new_call.calldata, initial_call.calldata);
    }
}

#[tokio::test(flavor = "multi_thread")]
#[cfg_attr(windows, ignore = "for some reason there's different rng")]
async fn test_shrink_fail_on_revert() {
    let runner = TEST_DATA_DEFAULT
        .runner_with_invariant_config_and_seed(
            U256::from(119u32),
            TestInvariantConfig {
                runs: 1,
                depth: 100,
                fail_on_revert: true,
                ..TestInvariantConfig::default()
            },
        )
        .await;

    let filter = SolidityTestFilter::new(
        ".*",
        ".*",
        ".*fuzz/invariant/common/InvariantShrinkFailOnRevert.t.sol",
    );

    match get_counterexample!(runner, filter) {
        CounterExample::Single(_) => panic!("CounterExample should be a sequence."),
        CounterExample::Sequence(sequence) => {
            // ensure shrinks to sequence of 10
            assert_eq!(sequence.len(), 10);
        }
    };
}

#[tokio::test(flavor = "multi_thread")]
async fn test_invariant_preserve_state() {
    let filter = SolidityTestFilter::new(
        ".*",
        ".*",
        ".*fuzz/invariant/common/InvariantPreserveState.t.sol",
    );
    let runner = TEST_DATA_DEFAULT
        .runner_with_invariant_config(TestInvariantConfig {
            fail_on_revert: true,
            ..TestInvariantConfig::default()
        })
        .await;
    let results = runner.test_collect(filter).await;
    assert_multiple(
        &results,
        BTreeMap::from([(
            "default/fuzz/invariant/common/InvariantPreserveState.t.sol:InvariantPreserveState",
            vec![(
                "invariant_preserve_state()",
                false,
                Some("EvmError: Revert".into()),
                None,
                None,
            )],
        )]),
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn test_invariant_with_address_fixture() {
    let runner = TEST_DATA_DEFAULT.runner().await;
    let results = runner
        .test_collect(SolidityTestFilter::new(
            ".*",
            ".*",
            ".*fuzz/invariant/common/InvariantCalldataDictionary.t.sol",
        ))
        .await;
    assert_multiple(
        &results,
        BTreeMap::from([(
            "default/fuzz/invariant/common/InvariantCalldataDictionary.t.sol:InvariantCalldataDictionary",
            vec![(
                "invariant_owner_never_changes()",
                false,
                Some("<empty revert data>".into()),
                None,
                None,
            )],
        )]),
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn test_invariant_assume_does_not_revert() {
    let filter =
        SolidityTestFilter::new(".*", ".*", ".*fuzz/invariant/common/InvariantAssume.t.sol");
    let runner = TEST_DATA_DEFAULT
        .runner_with_invariant_config(TestInvariantConfig {
            // Should not treat vm.assume as revert.
            fail_on_revert: true,
            ..TestInvariantConfig::default()
        })
        .await;
    let results = runner.test_collect(filter).await;
    assert_multiple(
        &results,
        BTreeMap::from([(
            "default/fuzz/invariant/common/InvariantAssume.t.sol:InvariantAssume",
            vec![("invariant_dummy()", true, None, None, None)],
        )]),
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn test_invariant_assume_respects_restrictions() {
    let filter =
        SolidityTestFilter::new(".*", ".*", ".*fuzz/invariant/common/InvariantAssume.t.sol");
    let runner = TEST_DATA_DEFAULT
        .runner_with_invariant_config(TestInvariantConfig {
            runs: 1,
            depth: 10,
            max_assume_rejects: 1,
            ..TestInvariantConfig::default()
        })
        .await;
    let results = runner.test_collect(filter).await;
    assert_multiple(
        &results,
        BTreeMap::from([(
            "default/fuzz/invariant/common/InvariantAssume.t.sol:InvariantAssume",
            vec![(
                "invariant_dummy()",
                false,
                Some("The `vm.assume` cheatcode rejected too many inputs (1 allowed)".into()),
                None,
                None,
            )],
        )]),
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn test_invariant_decode_custom_error() {
    let filter = SolidityTestFilter::new(
        ".*",
        ".*",
        ".*fuzz/invariant/common/InvariantCustomError.t.sol",
    );
    let runner = TEST_DATA_DEFAULT
        .runner_with_invariant_config(TestInvariantConfig {
            fail_on_revert: true,
            ..TestInvariantConfig::default()
        })
        .await;
    let results = runner.test_collect(filter).await;
    assert_multiple(
        &results,
        BTreeMap::from([(
            "default/fuzz/invariant/common/InvariantCustomError.t.sol:InvariantCustomError",
            vec![(
                "invariant_decode_error()",
                false,
                Some("InvariantCustomError(111, \"custom\")".into()),
                None,
                None,
            )],
        )]),
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn test_invariant_fuzzed_selected_targets() {
    let filter = SolidityTestFilter::new(
        ".*",
        ".*",
        ".*fuzz/invariant/target/FuzzedTargetContracts.t.sol",
    );
    let runner = TEST_DATA_DEFAULT
        .runner_with_invariant_config(TestInvariantConfig {
            fail_on_revert: true,
            ..TestInvariantConfig::default()
        })
        .await;
    let results = runner.test_collect(filter).await;
    assert_multiple(
        &results,
        BTreeMap::from([
            (
                "default/fuzz/invariant/target/FuzzedTargetContracts.t.sol:ExplicitTargetContract",
                vec![("invariant_explicit_target()", true, None, None, None)],
            ),
            (
                "default/fuzz/invariant/target/FuzzedTargetContracts.t.sol:DynamicTargetContract",
                vec![(
                    "invariant_dynamic_targets()",
                    false,
                    Some("revert: wrong target selector called".into()),
                    None,
                    None,
                )],
            ),
        ]),
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn test_invariant_fixtures() {
    let filter = SolidityTestFilter::new(
        ".*",
        ".*",
        ".*fuzz/invariant/common/InvariantFixtures.t.sol",
    );
    let runner = TEST_DATA_DEFAULT
        .runner_with_invariant_config(TestInvariantConfig {
            runs: 1,
            depth: 100,
            ..TestInvariantConfig::default()
        })
        .await;
    let results = runner.test_collect(filter).await;
    assert_multiple(
        &results,
        BTreeMap::from([(
            "default/fuzz/invariant/common/InvariantFixtures.t.sol:InvariantFixtures",
            vec![(
                "invariant_target_not_compromised()",
                false,
                Some("<empty revert data>".into()),
                None,
                None,
            )],
        )]),
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn test_invariant_scrape_values() {
    let filter = SolidityTestFilter::new(
        ".*",
        ".*",
        ".*fuzz/invariant/common/InvariantScrapeValues.t.sol",
    );
    let runner = TEST_DATA_DEFAULT
        .runner_with_invariant_config(TestInvariantConfig {
            runs: 50,
            depth: 300,
            fail_on_revert: true,
            ..TestInvariantConfig::default()
        })
        .await;

    let results = runner.test_collect(filter).await;
    assert_multiple(
        &results,
        BTreeMap::from([
            (
                "default/fuzz/invariant/common/InvariantScrapeValues.t.sol:FindFromReturnValueTest",
                vec![(
                    "invariant_value_not_found()",
                    false,
                    Some("revert: value from return found".into()),
                    None,
                    None,
                )],
            ),
            (
                "default/fuzz/invariant/common/InvariantScrapeValues.t.sol:FindFromLogValueTest",
                vec![(
                    "invariant_value_not_found()",
                    false,
                    Some("revert: value from logs found".into()),
                    None,
                    None,
                )],
            ),
        ]),
    );
}