use edr_stack_trace_tests::{known_failures, run, CompilerConfig, Optimizer};

/// The newest solc the corpus runs against; must be the newest version EDR
/// supports.
const LATEST_SOLC: semver::Version = semver::Version::new(0, 8, 35);

fn solc(version: semver::Version, optimizer: Option<Optimizer>) -> CompilerConfig {
    CompilerConfig::Solc { version, optimizer }
}

fn check(config: &CompilerConfig, groups: &[&str]) -> anyhow::Result<()> {
    known_failures::check(config, &run(config, groups)?)
}

#[test]
fn latest_solc_is_latest_supported() {
    assert_eq!(
        LATEST_SOLC,
        edr_instrument::LATEST_SUPPORTED_SOLIDITY_VERSION,
        "run the corpus against the newest solc EDR supports"
    );
}

#[test]
fn solc_0_6_3() -> anyhow::Result<()> {
    check(
        &solc(semver::Version::new(0, 6, 3), None),
        &["0_6", "version-independent"],
    )
}

#[test]
fn solc_0_7_6() -> anyhow::Result<()> {
    check(
        &solc(semver::Version::new(0, 7, 6), None),
        &["0_7", "version-independent"],
    )
}

#[test]
fn solc_latest() -> anyhow::Result<()> {
    check(&solc(LATEST_SOLC, None), &["0_8", "version-independent"])
}

#[test]
fn solc_latest_via_ir() -> anyhow::Result<()> {
    check(
        &solc(
            LATEST_SOLC,
            Some(Optimizer {
                via_ir: true,
                runs: 200,
            }),
        ),
        &["0_8", "version-independent"],
    )
}
