//! Benchmark for construcing a contract identifier.
//!
//! Steps to run:
//! 1. Check out https://github.com/NomicFoundation/forge-std/tree/js-benchmark-config
//!    locally (note the branch).
//! 2. In the `forge-std` repo root:
//! 2.1. `npm i`
//! 2.2. `npx hardhat compile`
//! 3. In the `crates/edr_solidity` directory:
//! 3.1. `export FORGE_STD_ARTIFACTS_DIR=/path/to/forge-std/artifacts`
//! 3.2. `cargo bench contracts_identifier`
use std::{fs, path::PathBuf, time::Duration};

use criterion::{black_box, criterion_group, criterion_main, Criterion};
use edr_solidity::{
    artifacts::BuildInfo,
    contract_decoder::{BuildInfoConfig, ContractDecoder},
};

fn load_build_info_config() -> anyhow::Result<BuildInfoConfig> {
    let artifacts_dir = std::env::var("FORGE_STD_ARTIFACTS_DIR")?;
    let build_info_dir = PathBuf::from(&artifacts_dir).join("build-info");

    let mut build_infos = Vec::new();
    for entry in fs::read_dir(build_info_dir)? {
        let entry = entry?;
        let path = entry.path();

        if path.is_file() && path.extension().and_then(|ext| ext.to_str()) == Some("json") {
            let contents = fs::read(&path)?;
            let build_info = serde_json::from_slice::<BuildInfo>(&contents)?;
            build_infos.push(build_info);
        }
    }

    println!("build infos len: {}", build_infos.len());

    Ok(BuildInfoConfig {
        build_infos: Some(build_infos),
        ignore_contracts: None,
    })
}

pub fn criterion_benchmark(c: &mut Criterion) {
    let build_info_config = load_build_info_config().expect("loads build info config");
    let contracts = &build_info_config
        .build_infos
        .as_ref()
        .expect("loaded build info")
        .first()
        .expect("there is at least one build info")
        .output
        .contracts;

    // Sanity check
    let total_contracts = contracts.iter().map(|(k, v)| v.len()).sum::<usize>();
    let min_contracts = 70;
    if total_contracts < min_contracts {
        panic!(
            "Expected at least {} contracts, instead it is {}",
            min_contracts, total_contracts
        );
    }

    c.bench_function("initialize_contracts_identifier", |b| {
        b.iter(|| ContractDecoder::new(black_box(&build_info_config)).unwrap())
    });
}

criterion_group!(name = benches; config = Criterion::default().measurement_time(Duration::from_secs(30)); targets = criterion_benchmark);
criterion_main!(benches);
