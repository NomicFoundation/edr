#![warn(missing_docs)]

//! Implementation of a JSON-RPC client for EVM-based blockchains that uses a
//! caching strategy based on finalized blocks and chain IDs.

/// Types for caching JSON-RPC responses
pub mod cache;
mod client;
/// Types for JSON-RPC error reporting.
pub mod error;
/// Types specific to JSON-RPC
pub mod jsonrpc;
/// HTTP proxy helpers.
pub mod proxy;

pub use self::client::*;
