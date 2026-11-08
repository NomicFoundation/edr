//! Compiler configurations and standard-JSON compilation of a case.

use std::{
    io::Write as _,
    path::{Path, PathBuf},
    process::{Command, Stdio},
};

use anyhow::Context as _;
use edr_solidity::artifacts::{
    solc::extract_solc_contract_metadata, BuildInfoConfig, CompilerInput, CompilerOutput,
    SolcBytecode,
};
use serde_json::{json, Value};

use crate::corpus::Case;

/// solc optimizer settings for a configuration.
#[derive(Clone, Copy, Debug)]
pub struct Optimizer {
    pub via_ir: bool,
    pub runs: u32,
}

/// A compiler and the settings every case is compiled with.
#[derive(Clone, Debug)]
pub enum CompilerConfig {
    Solc {
        version: semver::Version,
        optimizer: Option<Optimizer>,
    },
}

/// A compiled case: what the provider's decoder is built from, plus the raw
/// output for ABIs and bytecode.
pub(crate) struct Compilation {
    pub build_info: BuildInfoConfig,
    pub output: Value,
}

impl CompilerConfig {
    /// The name its known-failures file is stored under.
    pub fn name(&self) -> String {
        match self {
            CompilerConfig::Solc { version, optimizer } => match optimizer {
                None => format!("solc-{version}"),
                Some(Optimizer { via_ir: true, runs }) => format!("solc-{version}-via-ir-{runs}"),
                Some(Optimizer {
                    via_ir: false,
                    runs,
                }) => {
                    format!("solc-{version}-optimized-{runs}")
                }
            },
        }
    }

    pub fn solidity_version(&self) -> &semver::Version {
        match self {
            CompilerConfig::Solc { version, .. } => version,
        }
    }

    pub fn is_via_ir(&self) -> bool {
        match self {
            CompilerConfig::Solc { optimizer, .. } => optimizer.is_some_and(|o| o.via_ir),
        }
    }

    /// Whether line numbers are compared: only for unoptimized output.
    pub(crate) fn compares_lines(&self) -> bool {
        match self {
            CompilerConfig::Solc { optimizer, .. } => optimizer.is_none(),
        }
    }

    /// Installs the compiler if necessary and returns its path.
    pub(crate) fn install(&self) -> anyhow::Result<PathBuf> {
        match self {
            CompilerConfig::Solc { version, .. } => {
                let solc = foundry_compilers::solc::Solc::find_or_install(version)
                    .with_context(|| format!("installing solc {version}"))?;
                Ok(solc.solc)
            }
        }
    }

    pub(crate) fn compile(&self, compiler: &Path, case: &Case) -> anyhow::Result<Compilation> {
        let input = self.standard_json_input(case)?;
        let output = run_standard_json(compiler, &input)?;

        let CompilerConfig::Solc { version, .. } = self;
        let typed_input: CompilerInput = serde_json::from_value(input)?;
        let typed_output: CompilerOutput<SolcBytecode> =
            serde_json::from_value(output.clone()).context("deserializing compiler output")?;
        let identified_contracts =
            extract_solc_contract_metadata(version.to_string(), typed_input, typed_output)?;

        Ok(Compilation {
            build_info: BuildInfoConfig {
                identified_contracts,
                ignore_contracts: Some(true),
            },
            output,
        })
    }

    fn standard_json_input(&self, case: &Case) -> anyhow::Result<Value> {
        // Sources are keyed by file name, which test.json refers to them by.
        let mut sources = serde_json::Map::new();
        for path in &case.sources {
            let name = path
                .file_name()
                .context("source without a file name")?
                .to_string_lossy()
                .into_owned();
            let content = std::fs::read_to_string(path)
                .with_context(|| format!("reading {}", path.display()))?;
            sources.insert(name, json!({ "content": content }));
        }

        let CompilerConfig::Solc { optimizer, .. } = self;
        let mut settings = json!({
            "optimizer": match optimizer {
                None => json!({ "enabled": false }),
                Some(Optimizer { via_ir: true, runs }) => json!({
                    "enabled": true,
                    "runs": runs,
                    "details": { "yulDetails": { "optimizerSteps": "u" } },
                }),
                Some(Optimizer { via_ir: false, runs }) => json!({ "enabled": true, "runs": runs }),
            },
            "outputSelection": {
                "*": {
                    "*": ["abi", "evm.bytecode", "evm.deployedBytecode", "evm.methodIdentifiers"],
                    "": ["id", "ast"],
                },
            },
        });
        // Only set when true: older solc rejects the key.
        if self.is_via_ir()
            && let Some(settings) = settings.as_object_mut()
        {
            settings.insert("viaIR".to_owned(), json!(true));
        }

        Ok(json!({ "language": "Solidity", "sources": sources, "settings": settings }))
    }
}

fn run_standard_json(compiler: &Path, input: &Value) -> anyhow::Result<Value> {
    let mut child = Command::new(compiler)
        .arg("--standard-json")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .with_context(|| format!("running {}", compiler.display()))?;
    child
        .stdin
        .take()
        .expect("stdin is piped")
        .write_all(serde_json::to_string(input)?.as_bytes())?;
    let result = child.wait_with_output()?;
    anyhow::ensure!(
        result.status.success(),
        "{} exited with {}: {}",
        compiler.display(),
        result.status,
        String::from_utf8_lossy(&result.stderr)
    );

    let output: Value = serde_json::from_slice(&result.stdout)?;
    let errors: Vec<&str> = output
        .get("errors")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter(|error| error.get("severity").and_then(Value::as_str) == Some("error"))
        .map(|error| {
            error
                .get("formattedMessage")
                .and_then(Value::as_str)
                .unwrap_or("<no message>")
        })
        .collect();
    anyhow::ensure!(
        errors.is_empty(),
        "compilation failed:\n{}",
        errors.join("\n")
    );
    Ok(output)
}
