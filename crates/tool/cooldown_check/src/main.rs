mod allowlist;
mod cache;
mod config;
mod executor;
mod metadata;
mod registry;
mod resolver;
mod workspace;

use std::path::PathBuf;

use anyhow::anyhow;
use clap::Parser;
use clap_cargo::{Features, Manifest};
// use itertools::Itertools;
use log::{LevelFilter, Log, Record};

use crate::{config::Config, executor::run_pinning_flow, workspace::root_path};

const LOGGER: SimpleLogger = SimpleLogger;

#[derive(Parser)]
struct CliArgs {
    /// Enables verbose mode
    #[clap(short, long, takes_value = false)]
    verbose: bool,
}
#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = CliArgs::parse();
    check_dependencies(args.verbose).await
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
        LevelFilter::Info
    };
    log::set_logger(&LOGGER)
        .map(|()| log::set_max_level(level))
        .map_err(|error| anyhow!(error))
}

// ---------------- Repeated ------------------------------

async fn check_dependencies(verbose: bool) -> anyhow::Result<()> {
    init_logger(verbose)?;
    log::debug!("Cargo-cooldown check...");

    let config = Config::load()?;
    resolve_dependencies(config).await
}

async fn resolve_dependencies(config: Config) -> anyhow::Result<()> {
    let root_path = root_path()?;
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
    run_pinning_flow(&config, &manifest, &features).await
}
