//! A/B + profiling driver isolating the solx DWARF decode path.
//!
//! Builds the solx build model once, then repeatedly decodes every artifact's
//! debugInfo blob. No JSON parsing, AST processing, or cloning inside the
//! timed loop, so the measurement is DWARF work only. Complements
//! `benches/dwarf_decode.rs` with what criterion cannot report: per-stage RSS
//! checkpoints, per-blob timings for fitting a scaling exponent, and an
//! order-independent equivalence digest for A/B runs across revisions.
//!
//! Run with `cargo run --release -p edr_solidity --example dwarf_ab --
//! <input.json> <output.json> [iters]`; set `DWARF_AB_PER_BLOB=1` for
//! per-blob `BLOB\t<bytes>\t<ns>` lines.
//!
//! The input/output pair is solx standard JSON. To build one from an
//! arbitrary project, take `settings` from
//! `fixtures/solx_compiler_input_stack_trace_scenarios.json` (its
//! `outputSelection` includes `evm.{bytecode,deployedBytecode}.debugInfo` and
//! `ast`), inline the project's sources, and compile with
//! `solx --standard-json < input.json > output.json`. Check `errors` for
//! `severity == "error"` — a failed compile emits `contracts: {}` and the
//! driver would measure nothing.

use std::{hint::black_box, time::Instant};

use edr_primitives::hex;
use edr_solidity::{
    artifacts::{
        solx::SolxBuildModel, CompilerArtifact as _, CompilerInput, CompilerOutput, SolxBytecode,
    },
    build_model::{BuildModel as _, Instruction},
};

fn rss_line(label: &str) {
    let status = std::fs::read_to_string("/proc/self/status").unwrap_or_default();
    let get = |key: &str| {
        status.lines().find(|l| l.starts_with(key)).map_or_else(
            || "?".to_string(),
            |l| l.split_whitespace().nth(1).unwrap_or("?").to_string(),
        )
    };
    eprintln!(
        "RSS[{label}]\tVmRSS={} kB\tVmHWM={} kB",
        get("VmRSS:"),
        get("VmHWM:")
    );
}

/// Digest over everything a decode produces, for comparing revisions. Per-blob
/// digests are sorted before hashing: `output.contracts` is a `HashMap`, so
/// blob iteration order varies between processes.
fn equivalence_digest(all: &[Vec<Instruction>]) -> String {
    let mut blob_digests: Vec<String> = Vec::new();
    for insns in all {
        let mut digest = String::new();
        for i in insns {
            use std::fmt::Write as _;
            let loc = i.location.as_ref().map(|l| {
                let name = l
                    .file()
                    .map(|f| f.read().source_name.clone())
                    .unwrap_or_default();
                (name, l.offset, l.length)
            });
            let _ = write!(
                digest,
                "{}:{}:{:?}:{:?}:{:?}:{};",
                i.pc,
                i.opcode,
                i.jump_type,
                i.push_data,
                loc,
                i.inline_call_sites
                    .iter()
                    .map(|l| format!("{}+{}", l.offset, l.length))
                    .collect::<Vec<_>>()
                    .join(",")
            );
        }
        blob_digests.push(hex::encode(edr_primitives::keccak256(digest.as_bytes())));
    }
    blob_digests.sort_unstable();
    hex::encode(edr_primitives::keccak256(blob_digests.join(",").as_bytes()))
}

/// What the decoded instruction vectors themselves occupy, i.e. what a real
/// build-info load retains per contract.
fn report_retained(all: &[Vec<Instruction>], instructions: usize) {
    let inst_size = std::mem::size_of::<Instruction>();
    let push_heap: usize = all
        .iter()
        .flatten()
        .map(|i| i.push_data.as_ref().map_or(0, Vec::capacity))
        .sum();
    let call_sites: usize = all
        .iter()
        .flatten()
        .map(|i| i.inline_call_sites.len())
        .sum();
    eprintln!(
        "retained: {} vecs, {} instructions x {} B = {} kB inline, push_heap={} kB, call_sites={} entries",
        all.len(),
        instructions,
        inst_size,
        instructions * inst_size / 1024,
        push_heap / 1024,
        call_sites
    );
}

fn main() {
    let mut args = std::env::args().skip(1);
    let input_path = args
        .next()
        .expect("usage: dwarf_ab <input.json> <output.json> [iters]");
    let output_path = args
        .next()
        .expect("usage: dwarf_ab <input.json> <output.json> [iters]");
    let iters: u32 = args.next().map_or(20, |arg| {
        arg.parse().unwrap_or_else(|error| {
            panic!("iters must be a positive integer, got {arg:?}: {error}")
        })
    });
    assert!(iters > 0, "iters must be at least 1");

    rss_line("start");
    let input: CompilerInput =
        serde_json::from_str(&std::fs::read_to_string(&input_path).expect("input readable"))
            .expect("input parses");
    rss_line("input parsed");
    let output: CompilerOutput<SolxBytecode> =
        serde_json::from_str(&std::fs::read_to_string(&output_path).expect("output readable"))
            .expect("output parses");
    rss_line("output parsed");

    let model = SolxBuildModel::new(input, &output).expect("model builds");
    rss_line("model built");

    let mut items: Vec<(&SolxBytecode, Vec<u8>, bool)> = Vec::new();
    for contracts in output.contracts.values() {
        for contract in contracts.values() {
            for (artifact, is_deployment) in [
                (&contract.evm.bytecode, true),
                (&contract.evm.deployed_bytecode, false),
            ] {
                let Ok(code) = hex::decode(artifact.object()) else {
                    continue;
                };
                items.push((artifact, code, is_deployment));
            }
        }
    }

    let dwarf_bytes: usize = items.iter().map(|(a, ..)| a.debug_info.len() / 2).sum();

    // First pass: decode everything, retaining results, to see what the
    // instruction vectors themselves cost (this is what a real build-info
    // load retains per contract).
    let mut all: Vec<Vec<Instruction>> = Vec::new();
    let mut decoded = 0usize;
    let mut instructions = 0usize;
    for (artifact, code, is_deployment) in &items {
        if let Ok(insns) = model.decode_instructions(artifact, code, *is_deployment) {
            decoded += 1;
            instructions += insns.len();
            all.push(insns);
        }
    }
    rss_line("all decoded (retained)");
    eprintln!("DIGEST {}", equivalence_digest(&all));
    report_retained(&all, instructions);
    drop(all);
    rss_line("retained dropped");

    eprintln!(
        "items={} decoded={} instructions={} dwarf_bytes={} iters={}",
        items.len(),
        decoded,
        instructions,
        dwarf_bytes,
        iters
    );

    let start = Instant::now();
    for _ in 0..iters {
        for (artifact, code, is_deployment) in &items {
            if let Ok(insns) = model.decode_instructions(artifact, code, *is_deployment) {
                black_box(insns);
            }
        }
    }
    let elapsed = start.elapsed();
    rss_line("timed loop done");

    let per_iter = elapsed.as_secs_f64() / f64::from(iters);
    println!(
        "total={:.3}s per_pass={:.3}ms throughput={:.2}MB/s",
        elapsed.as_secs_f64(),
        per_iter * 1e3,
        (dwarf_bytes as f64 / 1e6) / per_iter,
    );

    if std::env::var("DWARF_AB_PER_BLOB").is_ok() {
        for (artifact, code, is_deployment) in &items {
            let bytes = artifact.debug_info.len() / 2;
            if bytes == 0 {
                continue;
            }
            let t = Instant::now();
            let mut ok = false;
            for _ in 0..iters {
                if let Ok(insns) = model.decode_instructions(artifact, code, *is_deployment) {
                    ok = true;
                    black_box(insns);
                }
            }
            if ok {
                let ns = t.elapsed().as_nanos() as f64 / f64::from(iters);
                println!("BLOB\t{bytes}\t{ns:.0}");
            }
        }
    }
}
