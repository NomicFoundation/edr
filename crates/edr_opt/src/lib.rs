#![warn(missing_docs)]

//! Optimism types
//!
//! Optimism types as needed by EDR. They are based on the same primitive types
//! as `revm`.

/// Optimism RPC types
pub mod rpc;

mod spec;
pub use self::spec::OptimismChainSpec;

/// Optimism transaction types
pub mod transaction;
