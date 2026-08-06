//! Benchmark for construcing a contract identifier.
//!
//! Steps to run:
//! 1. Check out <https://github.com/NomicFoundation/forge-std/tree/js-benchmark-config>
//!    locally (note the branch).
//!
//! 2. In the `forge-std` repo root:
//!
//!    2.1. `pnpm i`
//!    2.2. `npx hardhat compile`
//!
//! 3. In the `crates/edr_solidity` directory:
//!
//!    3.1. `export EDR_FORGE_STD_ARTIFACTS_DIR=/path/to/forge-std/artifacts`
//!    3.2. `cargo bench contracts_identifier`
use std::{fs, hint::black_box, path::PathBuf, time::Duration};

use criterion::{criterion_group, criterion_main, Criterion};
use edr_solidity::{
    artifacts::{solc::parse_solc_compiler_metadata, BuildInfoConfig},
    contract_decoder::ContractDecoder,
};

const FORGE_STD_ARTIFACTS_DIR: &str = "EDR_FORGE_STD_ARTIFACTS_DIR";

fn load_build_info_config() -> anyhow::Result<Option<BuildInfoConfig>> {
    let Ok(artifacts_dir) = std::env::var(FORGE_STD_ARTIFACTS_DIR) else {
        println!(
            "Skipping contracts identifier benchmark as {FORGE_STD_ARTIFACTS_DIR} environment variable is not set"
        );
        return Ok(None);
    };
    let build_info_dir = PathBuf::from(&artifacts_dir).join("build-info");

    let mut combined_identified_contracts = Vec::new();
    for entry in fs::read_dir(build_info_dir)? {
        let entry = entry?;
        let path = entry.path();

        if path.is_file() && path.extension().and_then(|ext| ext.to_str()) == Some("json") {
            let contents = fs::read(&path)?;
            let identified_contracts = parse_solc_compiler_metadata(contents.as_slice())?;
            combined_identified_contracts.extend(identified_contracts);
        }
    }

    Ok(Some(BuildInfoConfig {
        identified_contracts: combined_identified_contracts,
        ignore_contracts: None,
    }))
}

pub fn criterion_benchmark(c: &mut Criterion) {
    let Some(build_info_config) = load_build_info_config().expect("loads build info config") else {
        return;
    };

    // Sanity check
    let total_contracts = build_info_config.identified_contracts.len();
    let min_contracts = 70;
    assert!(
        total_contracts >= min_contracts,
        "Expected at least {min_contracts} contracts, instead it is {total_contracts}",
    );

    c.bench_function("initialize_contracts_identifier", |b| {
        b.iter(|| ContractDecoder::new(black_box(build_info_config.clone())));
    });
}

criterion_group!(name = benches; config = Criterion::default().measurement_time(Duration::from_secs(30)); targets = criterion_benchmark);
criterion_main!(benches);
