#![warn(missing_docs)]

//! Ethereum JSON-RPC client

/// Types for caching JSON-RPC responses
mod cache;
mod client;
/// Types specific to JSON-RPC
pub mod jsonrpc;
mod reqwest_error;

pub use self::{
    cache::{
        key::{ReadCacheKey, WriteCacheKey},
        CacheKeyHasher, CacheableMethod,
    },
    client::{RpcClient, RpcClientError},
    reqwest_error::{MiddlewareError, ReqwestError},
};
