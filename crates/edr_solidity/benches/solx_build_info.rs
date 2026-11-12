//! Instruction counts for loading solx and slang-compiler build info: model
//! construction plus DWARF decode of every bytecode section, as
//! `extract_{solx,slang}_contract_metadata` do when EDR loads a build info. CI
//! gates on these counts (see `.github/workflows/solx-build-info-benchmark.
//! yml`); `dwarf_decode` is the wall-time counterpart for local runs.
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
        slang::extract_slang_contract_metadata, solx::extract_solx_contract_metadata,
        CompilerInput, CompilerOutput, SolxBytecode,
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

#[library_benchmark]
#[bench::stack_trace_scenarios(common::slang_stack_trace_scenarios())]
fn extract_slang_metadata(
    (input, output): (CompilerInput, CompilerOutput<SolxBytecode>),
) -> Vec<IdentifiedContract> {
    black_box(
        extract_slang_contract_metadata("0.8.34".to_owned(), input, output)
            .expect("the committed fixture must load"),
    )
}

library_benchmark_group!(
    name = solx_build_info;
    benchmarks = extract_contract_metadata, extract_slang_metadata
);

main!(library_benchmark_groups = solx_build_info);
