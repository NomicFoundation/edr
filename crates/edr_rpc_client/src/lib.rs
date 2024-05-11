#![warn(missing_docs)]

//! Ethereum JSON-RPC client

mod client;
mod reqwest_error;

/// Types specific to JSON-RPC
pub mod jsonrpc;

pub use client::{RpcClient, RpcClientError};
