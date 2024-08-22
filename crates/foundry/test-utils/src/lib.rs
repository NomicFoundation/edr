#![warn(unused_crate_dependencies, unreachable_pub)]

// Macros useful for testing.

pub mod rpc;

pub mod fd_lock;

mod filter;

pub use filter::SolidityTestFilter;
// re-exports for convenience
pub use foundry_compilers;

/// Initializes tracing for Solidity tests.
pub fn init_tracing_for_solidity_tests() {
    let _ = tracing_subscriber::FmtSubscriber::builder()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .try_init();
}
