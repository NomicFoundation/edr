/// Types for caching block specifications.
pub mod block_spec;
pub(crate) mod chain_id;
/// Types for caching filters.
pub mod filter;
mod hasher;
/// Types for indexing the cache.
pub mod key;

use std::{
    fmt::Debug,
    io,
    path::{Path, PathBuf},
    time::Instant,
};

use serde::de::DeserializeOwned;

pub use self::hasher::KeyHasher;
use self::key::{ReadCacheKey, WriteCacheKey};
use crate::RpcClientError;

/// Trait for RPC method types that can be cached.
pub trait CacheableMethod: Sized {
    /// The type representing the cached method.
    type Cached<'method>: CachedMethod + TryFrom<&'method Self>
    where
        Self: 'method;

    /// Creates a method for requesting the block number.
    fn block_number_request() -> Self;

    /// Creates a method for requesting the chain ID.
    fn chain_id_request() -> Self;

    #[cfg(feature = "tracing")]
    /// Returns the name of the method.
    fn name(&self) -> &'static str;
}

/// Trait for RPC method types that will be cached to disk.
pub trait CachedMethod: Into<Option<Self::MethodWithResolvableBlockTag>> {
    /// The type representing a subset of methods containing a [`BlockTag`]
    /// which can be resolved to a block number.
    type MethodWithResolvableBlockTag: Clone + Debug;

    /// Resolves a block tag to a block number for the provided method.
    fn resolve_block_tag(method: Self::MethodWithResolvableBlockTag, block_number: u64) -> Self;

    /// Returns the instance's [`ReadCacheKey`] if it can be read from the
    /// cache.
    fn read_cache_key(self) -> Option<ReadCacheKey>;

    /// Returns the instance's [`WriteCacheKey`] if it can be written to the
    /// cache.
    fn write_cache_key(self) -> Option<WriteCacheKey<Self>>;
}

#[derive(Debug, Clone)]
pub(crate) struct CachedBlockNumber {
    pub block_number: u64,
    pub timestamp: Instant,
}

impl CachedBlockNumber {
    /// Creates a new instance with the current time.
    pub fn new(block_number: u64) -> Self {
        Self {
            block_number,
            timestamp: Instant::now(),
        }
    }
}

/// Wrapper for IO and JSON errors specific to the cache.
#[derive(thiserror::Error, Debug)]
pub enum Error {
    /// An IO error
    #[error(transparent)]
    Io(#[from] io::Error),
    /// A JSON parsing error
    #[error(transparent)]
    Json(#[from] serde_json::Error),
}

#[derive(Debug, Clone)]
pub(crate) struct Response {
    pub value: serde_json::Value,
    pub path: PathBuf,
}

impl Response {
    pub async fn parse<T: DeserializeOwned>(self) -> Result<T, RpcClientError> {
        match serde_json::from_value(self.value.clone()) {
            Ok(result) => Ok(result),
            Err(error) => {
                // Remove the file from cache if the contents don't match the expected type.
                // This can happen for example if a new field is added to a type.
                remove_from_cache(&self.path).await?;
                Err(RpcClientError::InvalidResponse {
                    response: self.value.to_string(),
                    expected_type: std::any::type_name::<T>(),
                    error,
                })
            }
        }
    }
}

/// Don't fail the request, just log an error if we fail to read/write from
/// cache.
pub(crate) fn log_error(cache_key: &str, message: &'static str, error: impl Into<Error>) {
    let cache_error = RpcClientError::CacheError {
        message: message.to_string(),
        cache_key: cache_key.to_string(),
        error: error.into(),
    };
    log::error!("{cache_error}");
}

pub(crate) async fn remove_from_cache(path: &Path) -> Result<(), RpcClientError> {
    match tokio::fs::remove_file(path).await {
        Ok(_) => Ok(()),
        Err(error) => {
            log_error(
                path.to_str().unwrap_or("<invalid UTF-8>"),
                "failed to remove from RPC response cache",
                error,
            );
            Ok(())
        }
    }
}
