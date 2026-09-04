//! Benchmarks for the solx DWARF decode path.
//!
//! `decode_instructions` re-parses hex → ELF → DWARF on every call (it runs
//! once per contract bytecode section at build-info load), so each iteration
//! measures the full per-blob cost: gimli DIE/attribute parsing, addr2line
//! context construction, and the per-PC location work.
//!
//! Always runs against the committed scenarios fixture. That corpus tops out
//! at ~13 KB blobs while decode time grows ~bytes^1.7, so a null result here
//! does not transfer to large projects. To also cover the expensive regime,
//! point `EDR_DWARF_BENCH_DIR` at a directory of `<name>.input.json` /
//! `<name>.output.json` solx standard-JSON pairs; each pair becomes its own
//! corpus. See `examples/dwarf_ab.rs` for how to build such a pair and for
//! per-blob scaling and RSS measurements that criterion does not provide.
use std::{fs, hint::black_box, path::PathBuf, time::Duration};

use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use edr_primitives::hex;
use edr_solidity::{
    artifacts::{
        solx::SolxBuildModel, CompilerArtifact as _, CompilerInput, CompilerOutput, SolxBytecode,
    },
    build_model::BuildModel as _,
};

const CORPUS_DIR_VAR: &str = "EDR_DWARF_BENCH_DIR";

/// One DWARF-carrying bytecode section that decodes successfully.
struct Blob {
    source: String,
    contract: String,
    is_deployment: bool,
    artifact: SolxBytecode,
    /// The hex-decoded bytecode.
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
                    let Ok(code) = hex::decode(artifact.object()) else {
                        continue;
                    };
                    // Probe once so the timed loop only sees blobs that
                    // decode; unlinked or truncated sections error early and
                    // would understate the per-pass cost.
                    if model
                        .decode_instructions(artifact, &code, is_deployment)
                        .is_err()
                    {
                        continue;
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
        assert!(
            !blobs.is_empty(),
            "corpus '{name}' has no decodable DWARF blobs"
        );

        let dwarf_bytes = blobs.iter().map(Blob::dwarf_bytes).sum();
        println!(
            "corpus '{name}': {} decodable DWARF blobs, {dwarf_bytes} bytes of DWARF",
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

/// The committed scenarios fixture. Mirrors the fixture setup in
/// `debug_info::dwarf`'s tests: the live source is spliced into the input,
/// which is valid because `Scenarios.t.sol` is append-only between fixture
/// regenerations.
fn committed_corpus() -> Corpus {
    // Each side of an A/B builds its corpus from its own revision, so a
    // change that makes blobs fail to decode would shrink the timed work on
    // one side only and read as a speedup. Pin what the committed fixture
    // yields today (96 blobs, ~490 KB; the byte count moves slightly per
    // regeneration because solx embeds the build directory). A regeneration
    // that lands below these is a signal to investigate, not to relax the
    // floor (see fixtures/README.md).
    const MIN_BLOBS: usize = 96;
    const MIN_DWARF_BYTES: u64 = 480_000;

    let mut input: CompilerInput = serde_json::from_str(include_str!(
        "../fixtures/solx_compiler_input_scenarios.json"
    ))
    .expect("solx_compiler_input_scenarios.json must parse");
    input
        .sources
        .get_mut("project/contracts/Scenarios.t.sol")
        .expect("scenarios input must contain Scenarios.t.sol")
        .content = include_str!("../fixtures/sources/Scenarios.t.sol").to_string();

    let output: CompilerOutput<SolxBytecode> = serde_json::from_str(include_str!(
        "../fixtures/solx_compiler_output_scenarios.json"
    ))
    .expect("solx_compiler_output_scenarios.json must parse");

    let corpus = Corpus::new("scenarios".to_string(), input, output);
    assert!(
        corpus.blobs.len() >= MIN_BLOBS,
        "scenarios fixture yields {} decodable DWARF blobs, expected at least {MIN_BLOBS}",
        corpus.blobs.len()
    );
    assert!(
        corpus.dwarf_bytes >= MIN_DWARF_BYTES,
        "scenarios fixture yields {} bytes of DWARF, expected at least {MIN_DWARF_BYTES}",
        corpus.dwarf_bytes
    );
    corpus
}

fn corpora_from_env() -> Vec<Corpus> {
    let Ok(dir) = std::env::var(CORPUS_DIR_VAR) else {
        println!(
            "{CORPUS_DIR_VAR} not set; benchmarking only the committed scenarios fixture (cheap regime, blobs <= ~13 KB)"
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
                    .expect("probed decode must succeed");
                black_box(instructions);
            }
        });
    });

    // The largest blob alone. The full pass is dominated by the corpus's many
    // small blobs and their fixed per-blob costs, and because decode time
    // grows super-linearly (~bytes^1.7) the cheap regime does not predict the
    // expensive one, so the latter gets its own bench. The blob's size goes
    // into the throughput, not the id: a fixture regeneration that changes it
    // must not rename the bench out from under `--baseline`.
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
                .expect("probed decode must succeed");
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
