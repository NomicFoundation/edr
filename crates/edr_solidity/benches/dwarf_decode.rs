//! Benchmarks for the solx DWARF decode path.
//!
//! `decode_instructions` re-parses hex → ELF → DWARF on every call (it runs
//! once per contract bytecode section at build-info load), so each iteration
//! measures the full per-blob cost: gimli DIE/attribute parsing, addr2line
//! context construction, and the per-PC location work.
//!
//! Always runs against the committed stack-trace scenarios fixture. That
//! corpus tops out at 3.6 KB blobs while decode time grows ~bytes^1.7, so a
//! null result here does not transfer to large projects. To also cover the
//! expensive regime, point `EDR_DWARF_BENCH_DIR` at a directory of
//! `<name>.input.json` / `<name>.output.json` solx standard-JSON pairs; each
//! pair becomes its own corpus. The book's benchmark page has the recipe.
mod common;

use std::{fs, hint::black_box, path::PathBuf, time::Duration};

use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use edr_solidity::{
    artifacts::{
        solx::SolxBuildModel, CompilerArtifact as _, CompilerInput, CompilerOutput, SolxBytecode,
    },
    build_model::BuildModel as _,
    library_utils::{get_library_address_positions, normalize_compiler_output_bytecode},
};

const CORPUS_DIR_VAR: &str = "EDR_DWARF_BENCH_DIR";

/// One DWARF-carrying bytecode section.
struct Blob {
    source: String,
    contract: String,
    is_deployment: bool,
    artifact: SolxBytecode,
    /// The bytecode with library placeholders zeroed, as production decodes it.
    code: Vec<u8>,
}

impl Blob {
    fn dwarf_bytes(&self) -> u64 {
        // `debug_info` is un-prefixed hex, so two characters per byte.
        u64::try_from(self.artifact.debug_info.len() / 2).expect("blob size fits in u64")
    }
}

struct Corpus {
    name: String,
    model: SolxBuildModel,
    /// Sorted by `(source, contract, is_deployment)`: `output.contracts` is a
    /// `HashMap`, so without this both sides of an A/B would traverse the
    /// blobs in different orders.
    blobs: Vec<Blob>,
    dwarf_bytes: u64,
}

impl Corpus {
    fn new(name: String, input: CompilerInput, output: CompilerOutput<SolxBytecode>) -> Self {
        let model = SolxBuildModel::new(input, &output)
            .unwrap_or_else(|error| panic!("corpus '{name}' must build a model: {error}"));

        let mut blobs = Vec::new();
        for (source, contracts) in &output.contracts {
            for (contract, compiled) in contracts {
                for (artifact, is_deployment) in [
                    (&compiled.evm.bytecode, true),
                    (&compiled.evm.deployed_bytecode, false),
                ] {
                    if artifact.debug_info.is_empty() {
                        continue;
                    }
                    let code = normalize_compiler_output_bytecode(
                        artifact.object().to_owned(),
                        &get_library_address_positions(artifact),
                    )
                    .unwrap_or_else(|error| {
                        panic!("{source}:{contract} bytecode must normalize: {error}")
                    });
                    if let Err(error) = model.decode_instructions(artifact, &code, is_deployment) {
                        panic!("{source}:{contract} must decode: {error}");
                    }
                    blobs.push(Blob {
                        source: source.clone(),
                        contract: contract.clone(),
                        is_deployment,
                        artifact: artifact.clone(),
                        code,
                    });
                }
            }
        }
        blobs.sort_by(|a, b| {
            (&a.source, &a.contract, a.is_deployment).cmp(&(
                &b.source,
                &b.contract,
                b.is_deployment,
            ))
        });
        assert!(!blobs.is_empty(), "corpus '{name}' has no DWARF blobs");

        let dwarf_bytes = blobs.iter().map(Blob::dwarf_bytes).sum();
        println!(
            "corpus '{name}': {} DWARF blobs, {dwarf_bytes} bytes of DWARF",
            blobs.len()
        );

        Self {
            name,
            model,
            blobs,
            dwarf_bytes,
        }
    }
}

fn committed_corpus() -> Corpus {
    // Each A/B side builds its corpus from its own revision, so blobs that stop
    // being read on one side would show up as a speedup.
    const BLOBS: usize = 76;

    let (input, output) = common::stack_trace_scenarios();
    let corpus = Corpus::new("stack_trace_scenarios".to_string(), input, output);
    assert_eq!(
        corpus.blobs.len(),
        BLOBS,
        "stack-trace fixture has 76 DWARF-carrying bytecode sections"
    );
    corpus
}

fn corpora_from_env() -> Vec<Corpus> {
    let Ok(dir) = std::env::var(CORPUS_DIR_VAR) else {
        println!(
            "{CORPUS_DIR_VAR} not set; benchmarking only the committed stack-trace scenarios fixture (cheap regime, blobs <= 3.6 KB)"
        );
        return Vec::new();
    };

    let mut corpora = Vec::new();
    for entry in
        fs::read_dir(&dir).unwrap_or_else(|error| panic!("{CORPUS_DIR_VAR}={dir}: {error}"))
    {
        let input_path = entry.expect("directory entry must be readable").path();
        let Some(name) = input_path
            .file_name()
            .and_then(|name| name.to_str())
            .and_then(|name| name.strip_suffix(".input.json"))
        else {
            continue;
        };
        let output_path = PathBuf::from(&dir).join(format!("{name}.output.json"));

        let input: CompilerInput = serde_json::from_str(
            &fs::read_to_string(&input_path)
                .unwrap_or_else(|error| panic!("{}: {error}", input_path.display())),
        )
        .unwrap_or_else(|error| panic!("{}: {error}", input_path.display()));
        let output: CompilerOutput<SolxBytecode> = serde_json::from_str(
            &fs::read_to_string(&output_path)
                .unwrap_or_else(|error| panic!("{}: {error}", output_path.display())),
        )
        .unwrap_or_else(|error| panic!("{}: {error}", output_path.display()));

        corpora.push(Corpus::new(name.to_string(), input, output));
    }
    assert!(
        !corpora.is_empty(),
        "{CORPUS_DIR_VAR}={dir} contains no *.input.json files"
    );
    corpora
}

fn bench_corpus(c: &mut Criterion, corpus: &Corpus) {
    let mut group = c.benchmark_group("dwarf_decode");

    group.throughput(Throughput::Bytes(corpus.dwarf_bytes));
    group.bench_function(BenchmarkId::new("full_pass", &corpus.name), |b| {
        b.iter(|| {
            for blob in &corpus.blobs {
                let instructions = corpus
                    .model
                    .decode_instructions(&blob.artifact, &blob.code, blob.is_deployment)
                    .expect("decoded once when the corpus was built");
                black_box(instructions);
            }
        });
    });

    // One blob's cost undiluted by the rest, so a per-blob regression is not
    // averaged away. Its size goes in the throughput, not the id, which a
    // fixture regeneration must not rename out from under `--baseline`.
    let largest = corpus
        .blobs
        .iter()
        .max_by_key(|blob| blob.dwarf_bytes())
        .expect("corpus is non-empty");
    group.throughput(Throughput::Bytes(largest.dwarf_bytes()));
    group.bench_function(BenchmarkId::new("largest_blob", &corpus.name), |b| {
        b.iter(|| {
            let instructions = corpus
                .model
                .decode_instructions(&largest.artifact, &largest.code, largest.is_deployment)
                .expect("decoded once when the corpus was built");
            black_box(instructions);
        });
    });

    group.finish();
}

pub fn criterion_benchmark(c: &mut Criterion) {
    bench_corpus(c, &committed_corpus());
    for corpus in corpora_from_env() {
        bench_corpus(c, &corpus);
    }
}

criterion_group!(
    name = benches;
    config = Criterion::default().measurement_time(Duration::from_secs(20)).sample_size(20);
    targets = criterion_benchmark
);
criterion_main!(benches);
