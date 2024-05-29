use std::{
    fmt::Debug,
    io,
    marker::PhantomData,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
    time::Duration,
};

use edr_eth::{
    block::{block_time, is_safe_block_number, IsSafeBlockNumberArgs},
    U64,
};
use futures::{future, TryFutureExt};
use hyper::header::HeaderValue;
pub use hyper::{header, HeaderMap};
use reqwest::Client as HttpClient;
use reqwest_middleware::{ClientBuilder as HttpClientBuilder, ClientWithMiddleware};
use reqwest_retry::{policies::ExponentialBackoff, RetryTransientMiddleware};
#[cfg(feature = "tracing")]
use reqwest_tracing::TracingMiddleware;
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use tokio::sync::{OnceCell, RwLock};
use uuid::Uuid;

use crate::{
    cache::{
        self,
        chain_id::chain_id_from_url,
        key::{
            CacheKeyForUncheckedBlockNumber, CacheKeyForUnresolvedBlockTag, ReadCacheKey,
            ResolvedSymbolicTag, WriteCacheKey,
        },
        remove_from_cache, CacheableMethod, CachedBlockNumber, CachedMethod,
    },
    jsonrpc, MiddlewareError, ReqwestError,
};

const RPC_CACHE_DIR: &str = "rpc_cache";
const TMP_DIR: &str = "tmp";
// Retry parameters for rate limited requests.
const EXPONENT_BASE: u32 = 2;
const MIN_RETRY_INTERVAL: Duration = Duration::from_secs(1);
const MAX_RETRY_INTERVAL: Duration = Duration::from_secs(32);
const MAX_RETRIES: u32 = 9;

/// Specialized error types
#[derive(Debug, thiserror::Error)]
pub enum RpcClientError {
    /// The message could not be sent to the remote node
    #[error(transparent)]
    FailedToSend(MiddlewareError),

    /// The remote node failed to reply with the body of the response
    #[error("The response text was corrupted: {0}.")]
    CorruptedResponse(ReqwestError),

    /// The server returned an error code.
    #[error("The Http server returned error status code: {0}")]
    HttpStatus(ReqwestError),

    /// The request cannot be serialized as JSON.
    #[error(transparent)]
    InvalidJsonRequest(serde_json::Error),

    /// The server returned an invalid JSON-RPC response.
    #[error("Response '{response}' failed to parse with expected type '{expected_type}', due to error: '{error}'")]
    InvalidResponse {
        /// The response text
        response: String,
        /// The expected type of the response
        expected_type: &'static str,
        /// The parse error
        error: serde_json::Error,
    },

    /// The server returned an invalid JSON-RPC id.
    #[error("The server returned an invalid id: '{id:?}' in response: '{response}'")]
    InvalidId {
        /// The response text
        response: String,
        /// The invalid id
        id: jsonrpc::Id,
    },

    /// Invalid URL format
    #[error(transparent)]
    InvalidUrl(#[from] url::ParseError),

    /// The JSON-RPC returned an error.
    #[error("{error}. Request: {request}")]
    JsonRpcError {
        /// The JSON-RPC error
        error: jsonrpc::Error,
        /// The request JSON
        request: String,
    },

    /// There was a problem with the local cache.
    #[error("{message} for '{cache_key}' with error: '{error}'")]
    CacheError {
        /// Description of the cache error
        message: String,
        /// The cache key for the error
        cache_key: String,
        /// The underlying error
        error: cache::Error,
    },

    /// Failed to join a tokio task.
    #[error(transparent)]
    JoinError(#[from] tokio::task::JoinError),
}

/// A client for executing RPC methods on a remote Ethereum node.
/// The client caches responses based on chain id, so it's important to not use
/// it with local nodes.
#[derive(Debug)]
pub struct RpcClient<MethodT: CacheableMethod + Serialize> {
    url: url::Url,
    chain_id: OnceCell<u64>,
    cached_block_number: RwLock<Option<CachedBlockNumber>>,
    client: ClientWithMiddleware,
    next_id: AtomicU64,
    rpc_cache_dir: PathBuf,
    tmp_dir: PathBuf,
    _phantom: PhantomData<MethodT>,
}

impl<MethodT: CacheableMethod + Serialize> RpcClient<MethodT> {
    /// Create a new instance, given a remote node URL.
    /// The cache directory is the global EDR cache directory configured by the
    /// user.
    pub fn new(
        url: &str,
        cache_dir: PathBuf,
        extra_headers: Option<HeaderMap>,
    ) -> Result<Self, RpcClientError> {
        let retry_policy = ExponentialBackoff::builder()
            .retry_bounds(MIN_RETRY_INTERVAL, MAX_RETRY_INTERVAL)
            .base(EXPONENT_BASE)
            .build_with_max_retries(MAX_RETRIES);

        let mut headers = extra_headers.unwrap_or_default();
        headers.append(
            header::CONTENT_TYPE,
            HeaderValue::from_static("application/json"),
        );
        headers.append(
            header::USER_AGENT,
            HeaderValue::from_str(&format!("edr {}", env!("CARGO_PKG_VERSION")))
                .expect("Version string is valid header value"),
        );

        let client = HttpClient::builder()
            .default_headers(headers)
            .build()
            .expect("Default construction nor setting default headers can cause an error");

        #[cfg(feature = "tracing")]
        let client = HttpClientBuilder::new(client)
            .with(TracingMiddleware::default())
            .with(RetryTransientMiddleware::new_with_policy(retry_policy))
            .build();
        #[cfg(not(feature = "tracing"))]
        let client = HttpClientBuilder::new(client)
            .with(RetryTransientMiddleware::new_with_policy(retry_policy))
            .build();

        let rpc_cache_dir = cache_dir.join(RPC_CACHE_DIR);
        // We aren't using the system temporary directories as they may be on a
        // different a file system which would cause the rename call later to
        // fail.
        let tmp_dir = rpc_cache_dir.join(TMP_DIR);

        Ok(RpcClient {
            url: url.parse()?,
            chain_id: OnceCell::new(),
            cached_block_number: RwLock::new(None),
            client,
            next_id: AtomicU64::new(0),
            rpc_cache_dir: cache_dir.join(RPC_CACHE_DIR),
            tmp_dir,
            _phantom: PhantomData,
        })
    }

    fn parse_response_str<SuccessT: DeserializeOwned>(
        response: String,
    ) -> Result<jsonrpc::Response<SuccessT>, RpcClientError> {
        serde_json::from_str(&response).map_err(|error| RpcClientError::InvalidResponse {
            response,
            expected_type: std::any::type_name::<jsonrpc::Response<SuccessT>>(),
            error,
        })
    }

    async fn retry_on_sporadic_failure<T: DeserializeOwned>(
        &self,
        error: jsonrpc::Error,
        request: SerializedRequest,
    ) -> Result<T, RpcClientError> {
        let is_missing_trie_node_error =
            error.code == -32000 && error.message.to_lowercase().contains("missing trie node");

        let result = if is_missing_trie_node_error {
            self.send_request_body(&request)
                .await
                .and_then(Self::parse_response_str)?
                .data
                .into_result()
        } else {
            Err(error)
        };

        result.map_err(|error| RpcClientError::JsonRpcError {
            error,
            request: request.to_json_string(),
        })
    }

    async fn make_cache_path(&self, cache_key: &str) -> Result<PathBuf, RpcClientError> {
        let chain_id = self.chain_id().await?;

        let host = self.url.host_str().unwrap_or("unknown-host");
        let remote = if let Some(port) = self.url.port() {
            // Include the port if it's not the default port for the protocol.
            format!("{host}_{port}")
        } else {
            host.to_string()
        };

        // We use different directories for each remote node, to avoid storing invalid
        // data in case the remote is forked chain which can happen with remotes
        // running locally.
        let directory = self.rpc_cache_dir.join(remote).join(chain_id.to_string());

        ensure_cache_directory(&directory, cache_key).await?;

        let path = Path::new(&directory).join(format!("{cache_key}.json"));
        Ok(path)
    }

    #[cfg_attr(feature = "tracing", tracing::instrument(level = "trace", skip_all))]
    async fn read_response_from_cache(
        &self,
        cache_key: &ReadCacheKey,
    ) -> Result<Option<cache::Response>, RpcClientError> {
        let path = self.make_cache_path(cache_key.as_ref()).await?;
        match tokio::fs::read_to_string(&path).await {
            Ok(contents) => match serde_json::from_str(&contents) {
                Ok(value) => Ok(Some(cache::Response { value, path })),
                Err(error) => {
                    cache::log_error(
                        cache_key.as_ref(),
                        "failed to deserialize item from RPC response cache",
                        error,
                    );
                    remove_from_cache(&path).await?;
                    Ok(None)
                }
            },
            Err(error) => {
                match error.kind() {
                    io::ErrorKind::NotFound => (),
                    _ => cache::log_error(
                        cache_key.as_ref(),
                        "failed to read from RPC response cache",
                        error,
                    ),
                }
                Ok(None)
            }
        }
    }

    async fn try_from_cache(
        &self,
        cache_key: Option<&ReadCacheKey>,
    ) -> Result<Option<cache::Response>, RpcClientError> {
        if let Some(cache_key) = cache_key {
            self.read_response_from_cache(cache_key).await
        } else {
            Ok(None)
        }
    }

    async fn maybe_cached_block_number(&self) -> Result<Option<u64>, RpcClientError> {
        let cached_block_number = { self.cached_block_number.read().await.clone() };

        if let Some(cached_block_number) = cached_block_number {
            let delta = block_time(self.chain_id().await?);
            if cached_block_number.timestamp.elapsed() < delta {
                return Ok(Some(cached_block_number.block_number));
            }
        }

        Ok(None)
    }

    /// Caches a block number for the duration of the block time of the chain.
    #[cfg_attr(feature = "tracing", tracing::instrument(level = "trace", skip(self)))]
    async fn cached_block_number(&self) -> Result<u64, RpcClientError> {
        if let Some(cached_block_number) = self.maybe_cached_block_number().await? {
            return Ok(cached_block_number);
        }

        // Caches the block number as side effect.
        self.block_number().await
    }

    #[cfg_attr(feature = "tracing", tracing::instrument(level = "trace", skip(self)))]
    async fn validate_block_number(
        &self,
        safety_checker: CacheKeyForUncheckedBlockNumber,
    ) -> Result<Option<String>, RpcClientError> {
        let chain_id = self.chain_id().await?;
        let latest_block_number = self.cached_block_number().await?;
        Ok(safety_checker.validate_block_number(chain_id, latest_block_number))
    }

    async fn resolve_block_tag<ResultT>(
        &self,
        block_tag_resolver: CacheKeyForUnresolvedBlockTag<MethodT::Cached<'_>>,
        result: ResultT,
        resolve_block_number: impl Fn(ResultT) -> Option<u64>,
    ) -> Result<Option<String>, RpcClientError> {
        if let Some(block_number) = resolve_block_number(result) {
            if let Some(resolved_cache_key) = block_tag_resolver.resolve_block_tag(block_number) {
                return match resolved_cache_key {
                    ResolvedSymbolicTag::NeedsSafetyCheck(safety_checker) => {
                        self.validate_block_number(safety_checker).await
                    }
                    ResolvedSymbolicTag::Resolved(cache_key) => Ok(Some(cache_key)),
                };
            }
        }
        Ok(None)
    }

    async fn resolve_write_key<ResultT>(
        &self,
        method: &MethodT,
        result: ResultT,
        resolve_block_number: impl Fn(ResultT) -> Option<u64>,
    ) -> Result<Option<String>, RpcClientError> {
        let cached_method = MethodT::Cached::try_from(method).ok();

        if let Some(cache_key) = cached_method.and_then(CachedMethod::write_cache_key) {
            match cache_key {
                WriteCacheKey::NeedsSafetyCheck(safety_checker) => {
                    self.validate_block_number(safety_checker).await
                }
                WriteCacheKey::NeedsBlockTagResolution(block_tag_resolver) => {
                    self.resolve_block_tag(block_tag_resolver, result, resolve_block_number)
                        .await
                }
                WriteCacheKey::Resolved(cache_key) => Ok(Some(cache_key)),
            }
        } else {
            Ok(None)
        }
    }

    async fn try_write_response_to_cache<ResultT: Serialize>(
        &self,
        method: &MethodT,
        result: &ResultT,
        resolve_block_number: impl Fn(&ResultT) -> Option<u64>,
    ) -> Result<(), RpcClientError> {
        if let Some(cache_key) = self
            .resolve_write_key(method, result, resolve_block_number)
            .await?
        {
            self.write_response_to_cache(&cache_key, result).await?;
        }

        Ok(())
    }

    async fn write_response_to_cache(
        &self,
        cache_key: &str,
        result: impl Serialize,
    ) -> Result<(), RpcClientError> {
        let contents = serde_json::to_string(&result).expect(
            "result serializes successfully as it was just deserialized from a JSON string",
        );

        ensure_cache_directory(&self.tmp_dir, cache_key).await?;

        // 1. Write to a random temporary file first to avoid race conditions.
        let tmp_path = self.tmp_dir.join(Uuid::new_v4().to_string());
        match tokio::fs::write(&tmp_path, contents).await {
            Ok(_) => (),
            Err(error) => {
                cache::log_error(
                    cache_key,
                    "failed to write to tempfile for RPC response cache",
                    error,
                );
                return Ok(());
            }
        }

        // 2. Then move the temporary file to the cache path.
        // This is guaranteed to be atomic on Unix platforms.
        // There is no such guarantee on Windows, as there is no OS support for atomic
        // move before Windows 10, but Rust will drop support for earlier
        // versions of Windows in the future: <https://github.com/rust-lang/compiler-team/issues/651>. Hopefully the standard
        // library will adapt its `rename` implementation to use the new atomic move API
        // in Windows
        // 10. In any case, if a cache file is corrupted, we detect and remove it when
        //     reading it.
        let cache_path = self.make_cache_path(cache_key).await?;
        match tokio::fs::rename(&tmp_path, cache_path).await {
            Ok(_) => (),
            Err(error) => {
                cache::log_error(
                    cache_key,
                    "failed to rename temporary file for RPC response cache",
                    error,
                );
            }
        };

        // In case of many concurrent renames, files remain in the tmp dir on Windows.
        #[cfg(target_os = "windows")]
        match tokio::fs::remove_file(&tmp_path).await {
            Ok(_) => (),
            Err(error) => match error.kind() {
                io::ErrorKind::NotFound => (),
                _ => cache::log_error(
                    cache_key,
                    "failed to remove temporary file for RPC response cache",
                    error,
                ),
            },
        }

        Ok(())
    }

    async fn send_request_and_extract_result<SuccessT: DeserializeOwned>(
        &self,
        request: SerializedRequest,
    ) -> Result<SuccessT, RpcClientError> {
        future::ready(
            self.send_request_body(&request)
                .await
                .and_then(Self::parse_response_str)?
                .data
                .into_result(),
        )
        // We retry at the application level because Alchemy has sporadic failures that are returned
        // in the JSON-RPC layer
        .or_else(|error| async { self.retry_on_sporadic_failure(error, request).await })
        .await
    }

    #[cfg_attr(feature = "tracing", tracing::instrument(level = "trace", skip_all))]
    async fn send_request_body(
        &self,
        request_body: &SerializedRequest,
    ) -> Result<String, RpcClientError> {
        self.client
            .post(self.url.clone())
            .body(request_body.to_json_string())
            .send()
            .await
            .map_err(|err| RpcClientError::FailedToSend(err.into()))?
            .error_for_status()
            .map_err(|err| RpcClientError::HttpStatus(err.into()))?
            .text()
            .await
            .map_err(|err| RpcClientError::CorruptedResponse(err.into()))
    }

    fn serialize_request(&self, input: &MethodT) -> Result<SerializedRequest, RpcClientError> {
        let id = jsonrpc::Id::Num(self.next_id.fetch_add(1, Ordering::Relaxed));
        Self::serialize_request_with_id(input, id)
    }

    fn serialize_request_with_id(
        method: &MethodT,
        id: jsonrpc::Id,
    ) -> Result<SerializedRequest, RpcClientError> {
        let request = serde_json::to_value(jsonrpc::Request {
            version: jsonrpc::Version::V2_0,
            id,
            method,
        })
        .map_err(RpcClientError::InvalidJsonRequest)?;

        Ok(SerializedRequest(request))
    }

    /// Calls the provided JSON-RPC method and returns the result.
    pub async fn call<SuccessT: DeserializeOwned + Serialize>(
        &self,
        method: MethodT,
    ) -> Result<SuccessT, RpcClientError> {
        self.call_with_resolver(method, |_| None).await
    }

    /// Calls the provided JSON-RPC method, uses the provided resolver to
    /// resolve the result, and returns the result.
    pub async fn call_with_resolver<SuccessT: DeserializeOwned + Serialize>(
        &self,
        method: MethodT,
        resolve_block_number: impl Fn(&SuccessT) -> Option<u64>,
    ) -> Result<SuccessT, RpcClientError> {
        let cached_method = MethodT::Cached::try_from(&method).ok();
        let read_cache_key = cached_method.and_then(CachedMethod::read_cache_key);

        let request = self.serialize_request(&method)?;

        if let Some(cached_response) = self.try_from_cache(read_cache_key.as_ref()).await? {
            match cached_response.parse().await {
                Ok(result) => {
                    #[cfg(feature = "tracing")]
                    tracing::trace!("Cache hit: {}", method.name());
                    return Ok(result);
                }
                Err(error) => match error {
                    // In case of an invalid response from cache, we log it and continue to the
                    // remote call.
                    RpcClientError::InvalidResponse {
                        response,
                        expected_type,
                        error,
                    } => {
                        log::error!(
                            "Failed to deserialize item from RPC response cache. error: '{error}' expected type: '{expected_type}'. item: '{response}'");
                    }
                    // For other errors, return early.
                    _ => return Err(error),
                },
            }
        };

        #[cfg(feature = "tracing")]
        tracing::trace!("Cache miss: {}", method.name());

        let result: SuccessT = self.send_request_and_extract_result(request).await?;

        self.try_write_response_to_cache(&method, &result, &resolve_block_number)
            .await?;

        Ok(result)
    }

    // We have two different `call` methods to avoid creating recursive async
    // functions as the cached path calls `eth_chainId` without caching.
    #[cfg_attr(feature = "tracing", tracing::instrument(level = "trace", skip_all))]
    async fn call_without_cache<T: DeserializeOwned>(
        &self,
        method: MethodT,
    ) -> Result<T, RpcClientError> {
        let request = self.serialize_request(&method)?;

        self.send_request_and_extract_result(request).await
    }

    /// Calls `eth_blockNumber` and returns the block number.
    #[cfg_attr(feature = "tracing", tracing::instrument(level = "trace", skip(self)))]
    pub async fn block_number(&self) -> Result<u64, RpcClientError> {
        let block_number = self
            .call_without_cache::<U64>(MethodT::block_number_request())
            .await?
            .as_limbs()[0];

        {
            let mut write_guard = self.cached_block_number.write().await;
            *write_guard = Some(CachedBlockNumber::new(block_number));
        }
        Ok(block_number)
    }

    /// Whether the block number should be cached based on its depth.
    #[cfg_attr(feature = "tracing", tracing::instrument(level = "trace", skip(self)))]
    pub async fn is_cacheable_block_number(
        &self,
        block_number: u64,
    ) -> Result<bool, RpcClientError> {
        let chain_id = self.chain_id().await?;
        let latest_block_number = self.cached_block_number().await?;

        Ok(is_safe_block_number(IsSafeBlockNumberArgs {
            chain_id,
            latest_block_number,
            block_number,
        }))
    }

    /// Calls `eth_chainId` and returns the chain ID.
    #[cfg_attr(feature = "tracing", tracing::instrument(level = "trace", skip(self)))]
    pub async fn chain_id(&self) -> Result<u64, RpcClientError> {
        let chain_id = *self
            .chain_id
            .get_or_try_init(|| async {
                if let Some(chain_id) = chain_id_from_url(&self.url) {
                    Ok(chain_id)
                } else {
                    self.call_without_cache::<U64>(MethodT::chain_id_request())
                        .await
                        .map(|chain_id| chain_id.as_limbs()[0])
                }
            })
            .await?;
        Ok(chain_id)
    }
}

/// Ensure that the directory exists.
async fn ensure_cache_directory(
    directory: impl AsRef<Path>,
    cache_key: impl std::fmt::Display,
) -> Result<(), RpcClientError> {
    tokio::fs::DirBuilder::new()
        .recursive(true)
        .create(directory)
        .await
        .map_err(|error| RpcClientError::CacheError {
            message: "failed to create RPC response cache directory".to_string(),
            cache_key: cache_key.to_string(),
            error: error.into(),
        })
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[repr(transparent)]
#[serde(transparent)]
struct SerializedRequest(serde_json::Value);

impl SerializedRequest {
    fn to_json_string(&self) -> String {
        self.0.to_string()
    }
}
