//! Instruction counts for loading solx build info: model construction plus
//! DWARF decode of every bytecode section, as `extract_solx_contract_metadata`
//! does when EDR loads a build info. CI gates on these counts (see
//! `.github/workflows/dwarf-decode-benchmark.yml`); `dwarf_decode` is the
//! wall-time counterpart for local runs.
//!
//! Needs valgrind and a `gungraun-runner` matching the `gungraun`
//! dev-dependency: `cargo install gungraun-runner --version 0.19.4 --locked`.

#![expect(
    clippy::exit,
    reason = "gungraun's `main!` exits with the runner's status"
)]

mod common;

use std::hint::black_box;

use edr_solidity::{
    artifacts::{
        solx::extract_solx_contract_metadata, CompilerInput, CompilerOutput, SolxBytecode,
    },
    contracts_identifier::IdentifiedContract,
};
use gungraun::{library_benchmark, library_benchmark_group, main};

#[library_benchmark]
#[bench::stack_trace_scenarios(common::stack_trace_scenarios())]
fn extract_contract_metadata(
    (input, output): (CompilerInput, CompilerOutput<SolxBytecode>),
) -> Vec<IdentifiedContract> {
    black_box(
        extract_solx_contract_metadata("0.8.34".to_owned(), input, output)
            .expect("the committed fixture must load"),
    )
}

library_benchmark_group!(name = solx_build_info; benchmarks = extract_contract_metadata);

main!(library_benchmark_groups = solx_build_info);
