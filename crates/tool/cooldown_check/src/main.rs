mod allowlist;
mod cache;
mod config;
mod cooldown_failure;
mod executor;
mod registry;
mod resolver;
mod workspace;

use anyhow::anyhow;
use clap::Parser;
use log::{LevelFilter, Log, Record};

use crate::{executor::run_check_flow, workspace::Workspace};

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

async fn check_dependencies(verbose: bool) -> anyhow::Result<()> {
    init_logger(verbose)?;
    log::info!("Running cargo cooldown check");

    let workspace = Workspace::load()?;
    run_check_flow(workspace).await
}
