#![warn(missing_docs)]

//! Ethereum JSON-RPC client

/// Types for caching JSON-RPC responses
pub mod cache;
mod client;
/// Types specific to JSON-RPC
pub mod jsonrpc;
mod reqwest_error;

pub use self::{
    client::{RpcClient, RpcClientError},
    reqwest_error::{MiddlewareError, ReqwestError},
};
