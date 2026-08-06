//! Regenerates the solx compiler-output fixtures in
//! `crates/edr_solidity/fixtures` from their inputs.
//!
//! Splices each fixture's source files (from `fixtures/sources/`) into its
//! committed input JSON — whose `content` fields are deliberately empty —
//! runs `solx --standard-json`, and rewrites the output JSON with solx's
//! verbatim output. Run after bumping the solx version or editing a fixture
//! source, then re-run the consuming tests
//! (`cargo test -p edr_provider --features test-utils solx_stack_trace`).
//!
//! solx release binaries: <https://github.com/NomicFoundation/solx/releases>.
//!
//! The committed inputs pin `settings.optimizer.mode` explicitly (trace
//! shapes differ per mode) so a change to a default can't silently change
//! what the fixtures test. The pinned "1" matches hardhat-solx's default,
//! the pipeline these fixtures stand in for — not solx's own default "3".
//! The `stack_trace_scenarios_mode3` variant exists because mode-1 DWARF
//! is statement-attributed since solx 0.1.6, so only mode-3 artifacts
//! still reach the inference's declaration-attributed and unmapped-revert
//! compat paths.
//!
//! The `scenarios` fixture is NOT regenerable by this tool: its input also
//! depends on forge-std sources whose contents are scrubbed from the
//! committed JSON. It was generated from a hardhat project with
//! `@nomicfoundation/hardhat-solx` configured; regenerate it there and
//! re-scrub the non-fixture `content` fields.

use std::{
    io::Write,
    path::Path,
    process::{Command, Stdio},
};

use anyhow::{bail, Context};

use crate::update::{project_root, update, Mode};

struct Fixture {
    name: &'static str,
    input: &'static str,
    /// Source name in the compiler input -> file under `fixtures/sources/`.
    sources: &'static [(&'static str, &'static str)],
    output: &'static str,
}

const FIXTURES: &[Fixture] = &[
    Fixture {
        name: "counter",
        input: "solx_compiler_input.json",
        sources: &[("Counter.sol", "Counter.sol")],
        output: "solx_compiler_output.json",
    },
    Fixture {
        name: "stack_trace_scenarios",
        input: "solx_compiler_input_stack_trace_scenarios.json",
        sources: &[(
            "project/contracts/StackTraceScenarios.sol",
            "StackTraceScenarios.sol",
        )],
        output: "solx_compiler_output_stack_trace_scenarios.json",
    },
    Fixture {
        name: "stack_trace_scenarios_mode3",
        input: "solx_compiler_input_stack_trace_scenarios_mode3.json",
        sources: &[(
            "project/contracts/StackTraceScenarios.sol",
            "StackTraceScenarios.sol",
        )],
        output: "solx_compiler_output_stack_trace_scenarios_mode3.json",
    },
];

pub fn generate(solx: &Path) -> anyhow::Result<()> {
    let version = run_solx(solx, &["--version"], None)?;
    println!("using: {}", version.trim());

    let fixtures_dir = project_root().join("crates/edr_solidity/fixtures");
    for fixture in FIXTURES {
        generate_fixture(solx, &fixtures_dir, fixture)?;
    }
    Ok(())
}

fn generate_fixture(solx: &Path, fixtures_dir: &Path, fixture: &Fixture) -> anyhow::Result<()> {
    let input_path = fixtures_dir.join(fixture.input);
    let input = std::fs::read_to_string(&input_path)
        .with_context(|| format!("failed to read {}", input_path.display()))?;
    let mut compiler_input: serde_json::Value = serde_json::from_str(&input)
        .with_context(|| format!("failed to parse {}", input_path.display()))?;

    let sources = compiler_input
        .get_mut("sources")
        .and_then(serde_json::Value::as_object_mut)
        .with_context(|| format!("no `sources` object in {}", input_path.display()))?;
    for (source_name, source_file) in fixture.sources {
        let source_path = fixtures_dir.join("sources").join(source_file);
        let content = std::fs::read_to_string(&source_path)
            .with_context(|| format!("failed to read {}", source_path.display()))?;
        sources
            .get_mut(*source_name)
            .and_then(serde_json::Value::as_object_mut)
            .with_context(|| format!("no source `{source_name}` in {}", input_path.display()))?
            .insert("content".to_owned(), serde_json::Value::String(content));
    }
    let scrubbed: Vec<&String> = sources
        .iter()
        .filter(|(_, source)| source.get("content").and_then(serde_json::Value::as_str) == Some(""))
        .map(|(source_name, _)| source_name)
        .collect();
    if !scrubbed.is_empty() {
        bail!(
            "{}: sources without content, cannot compile: {scrubbed:?}",
            fixture.name
        );
    }

    let compiler_output = run_solx(
        solx,
        &["--standard-json"],
        Some(&serde_json::to_string(&compiler_input)?),
    )?;

    let parsed: serde_json::Value = serde_json::from_str(&compiler_output)
        .with_context(|| format!("{}: solx emitted invalid JSON", fixture.name))?;
    let errors: Vec<&serde_json::Value> = parsed
        .get("errors")
        .and_then(serde_json::Value::as_array)
        .map(|errors| {
            errors
                .iter()
                .filter(|error| {
                    error.get("severity").and_then(serde_json::Value::as_str) == Some("error")
                })
                .collect()
        })
        .unwrap_or_default();
    if !errors.is_empty() {
        let messages: Vec<String> = errors
            .iter()
            .map(|error| {
                error
                    .get("formattedMessage")
                    .and_then(serde_json::Value::as_str)
                    .map_or_else(|| error.to_string(), str::to_owned)
            })
            .collect();
        bail!(
            "{}: solx reported errors:\n{}",
            fixture.name,
            messages.join("\n")
        );
    }

    update(
        &fixtures_dir.join(fixture.output),
        &compiler_output,
        Mode::Overwrite,
    )
    .with_context(|| format!("{}: writing the compiler output", fixture.name))
}

fn run_solx(solx: &Path, args: &[&str], stdin: Option<&str>) -> anyhow::Result<String> {
    let mut child = Command::new(solx)
        .args(args)
        .stdin(if stdin.is_some() {
            Stdio::piped()
        } else {
            Stdio::null()
        })
        .stdout(Stdio::piped())
        .spawn()
        .with_context(|| format!("failed to run {}", solx.display()))?;
    // Feed stdin from a thread so a large input can't deadlock against the
    // child filling the stdout pipe; a write error surfaces as a non-zero
    // exit status below.
    let output = std::thread::scope(|scope| {
        if let Some(stdin_contents) = stdin {
            let mut child_stdin = child.stdin.take().expect("stdin is piped");
            scope.spawn(move || {
                let _ = child_stdin.write_all(stdin_contents.as_bytes());
            });
        }
        child.wait_with_output()
    })?;
    if !output.status.success() {
        bail!(
            "`{} {}` failed: {}",
            solx.display(),
            args.join(" "),
            output.status
        );
    }
    Ok(String::from_utf8(output.stdout)?)
}
