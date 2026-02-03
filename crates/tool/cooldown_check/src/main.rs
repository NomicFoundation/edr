mod metadata;
use std::{fs, path::PathBuf, process::Command, sync::OnceLock};

use anyhow::{anyhow, bail};
use clap::Parser;
use clap_cargo::{Features, Manifest};
// use itertools::Itertools;
use log::{LevelFilter, Log, Record};

use crate::metadata::read_metadata;

static WORKSPACE_ROOT_PATH: OnceLock<anyhow::Result<String>> = OnceLock::new();

const LOGGER: SimpleLogger = SimpleLogger;

#[derive(Parser)]
struct CliArgs {
    /// Enables verbose mode
    #[clap(short, long, takes_value = false)]
    verbose: bool,
}

fn main() -> anyhow::Result<()> {
    let args = CliArgs::parse();
    check_dependencies(args.verbose)
}

// TODO: this is repeated in op_chain_config_generator
// ---------------- Repeated ------------------------------
struct SimpleLogger;

impl Log for SimpleLogger {
    fn enabled(&self, metadata: &log::Metadata<'_>) -> bool {
        metadata.level() <= log::max_level()
    }

    fn log(&self, record: &Record<'_>) {
        if self.enabled(record.metadata()) {
            println!("{} - {}", record.level(), record.args());
        }
    }

    fn flush(&self) {}
}

pub fn init_logger(verbose: bool) -> anyhow::Result<()> {
    let level = if verbose {
        LevelFilter::Debug
    } else {
        LevelFilter::Off
    };
    log::set_logger(&LOGGER)
        .map(|()| log::set_max_level(level))
        .map_err(|error| anyhow!(error))
}

fn workspace_root() -> anyhow::Result<String> {
    let output = Command::new("cargo")
        .arg("metadata")
        .arg("--no-deps")
        .arg("--format-version")
        .arg("1")
        .output()?;

    if !output.status.success() {
        bail!(
            "cargo metadata failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    let stdout = String::from_utf8(output.stdout)?;
    let metadata: serde_json::Value = serde_json::from_str(&stdout)?;

    metadata
        .get("workspace_root")
        .and_then(|value| value.as_str().map(String::from))
        .ok_or(anyhow!("Could not find workspace_root in cargo metadata"))
}

// ---------------- Repeated ------------------------------

fn check_dependencies(verbose: bool) -> anyhow::Result<()> {
    init_logger(verbose)?;
    let root_path = WORKSPACE_ROOT_PATH
        .get_or_init(workspace_root)
        .as_ref()
        .map_err(|error| anyhow!("Could not determine EDR root path: {error}"))?;
    let cooldown_config_path = format!("{root_path}/.cargo/cooldown.toml");
    log::debug!("Cargo-cooldown check...");
    log::debug!("project cooldown config path: {cooldown_config_path}");

    let _cooldown_config = {
        let file_contents = fs::read_to_string(cooldown_config_path)?;
        let cooldown_config: CooldownConfig = toml::from_str(&file_contents)?;
        log::debug!("cooldown config: {cooldown_config:?}");
        cooldown_config
    };

    let features = {
        let mut features = Features::default();
        features.all_features = true;
        features
    };
    let manifest = {
        let mut manifest = Manifest::default();
        manifest.manifest_path = Some(PathBuf::from(root_path).join("Cargo.toml"));
        manifest
    };

    let metadata = read_metadata(&manifest, &features)?;
    log::debug!("Read metadata: {metadata:?}");
    Ok(())
}

#[derive(serde::Deserialize, serde::Serialize, Debug)]
struct CooldownConfig {
    cooldown_minutes: u64,
}
