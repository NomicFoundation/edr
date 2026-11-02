use edr_stack_trace_tests::{known_failures, run, CompilerConfig, Optimizer};

const SOLC_0_8: &[&str] = &["0_8", "version-independent"];

fn solc(version: &str, optimizer: Option<Optimizer>) -> CompilerConfig {
    CompilerConfig::Solc {
        version: semver::Version::parse(version).expect("valid version"),
        optimizer,
    }
}

#[test]
fn solc_0_8_35() -> anyhow::Result<()> {
    let config = solc("0.8.35", None);
    known_failures::check(&config, &run(&config, SOLC_0_8)?)
}

#[test]
fn solc_0_8_35_via_ir() -> anyhow::Result<()> {
    let config = solc(
        "0.8.35",
        Some(Optimizer {
            via_ir: true,
            runs: 200,
        }),
    );
    known_failures::check(&config, &run(&config, SOLC_0_8)?)
}
