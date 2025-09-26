//! Support for multiple RPC-endpoints

use std::{
    collections::BTreeMap,
    fmt,
    ops::Deref,
};

/// Container type for API endpoints, like various RPC endpoints
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RpcEndpoints {
    endpoints: BTreeMap<String, RpcEndpoint>,
}

impl RpcEndpoints {
    /// Creates a new list of endpoints
    pub fn new(
        endpoints: impl IntoIterator<Item = (impl Into<String>, impl Into<RpcEndpointType>)>,
    ) -> Self {
        Self {
            endpoints: endpoints
                .into_iter()
                .map(|(name, e)| match e.into() {
                    RpcEndpointType::String(url) => (name.into(), RpcEndpoint::new(url)),
                    RpcEndpointType::Config(config) => (name.into(), config),
                })
                .collect(),
        }
    }

    /// Returns `true` if this type doesn't contain any endpoints
    pub fn is_empty(&self) -> bool {
        self.endpoints.is_empty()
    }
}

impl Deref for RpcEndpoints {
    type Target = BTreeMap<String, RpcEndpoint>;

    fn deref(&self) -> &Self::Target {
        &self.endpoints
    }
}

/// RPC endpoint wrapper type
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RpcEndpointType {
    /// Raw Endpoint url string
    String(RpcEndpointUrl),
    /// Config object
    Config(RpcEndpoint),
}

impl RpcEndpointType {
    /// Returns the string variant
    pub fn as_endpoint_string(&self) -> Option<&RpcEndpointUrl> {
        match self {
            Self::String(url) => Some(url),
            Self::Config(_) => None,
        }
    }

    /// Returns the config variant
    pub fn as_endpoint_config(&self) -> Option<&RpcEndpoint> {
        match self {
            Self::Config(config) => Some(config),
            Self::String(_) => None,
        }
    }
}

impl fmt::Display for RpcEndpointType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::String(url) => url.fmt(f),
            Self::Config(config) => config.fmt(f),
        }
    }
}

/// Represents a single endpoint
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RpcEndpointUrl {
    /// A raw Url (ws, http)
    Url(String),
    /// An endpoint that contains at least one `${ENV_VAR}` placeholder
    ///
    /// **Note:** this contains the endpoint as is, like `https://eth-mainnet.alchemyapi.io/v2/${API_KEY}` or `${EPC_ENV_VAR}`
    Env(String),
}

impl RpcEndpointUrl {
    /// Returns the url variant
    pub fn as_url(&self) -> Option<&str> {
        match self {
            Self::Url(url) => Some(url),
            Self::Env(_) => None,
        }
    }

    /// Returns the env variant
    pub fn as_env(&self) -> Option<&str> {
        match self {
            Self::Env(val) => Some(val),
            Self::Url(_) => None,
        }
    }
}

impl fmt::Display for RpcEndpointUrl {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Url(url) => url.fmt(f),
            Self::Env(var) => var.fmt(f),
        }
    }
}

impl From<RpcEndpointUrl> for RpcEndpointType {
    fn from(endpoint: RpcEndpointUrl) -> Self {
        Self::String(endpoint)
    }
}

impl From<RpcEndpointUrl> for RpcEndpoint {
    fn from(endpoint: RpcEndpointUrl) -> Self {
        Self { endpoint, ..Default::default() }
    }
}

/// The auth token to be used for RPC endpoints
/// It works in the same way as the `RpcEndpoint` type, where it can be a raw string or a reference
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RpcAuth {
    Raw(String),
    Env(String),
}

impl fmt::Display for RpcAuth {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Raw(url) => url.fmt(f),
            Self::Env(var) => var.fmt(f),
        }
    }
}

// Rpc endpoint configuration
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RpcEndpointConfig {
    /// The number of retries.
    pub retries: Option<u32>,

    /// Initial retry backoff.
    pub retry_backoff: Option<u64>,

    /// The available compute units per second.
    ///
    /// See also <https://docs.alchemy.com/reference/compute-units#what-are-cups-compute-units-per-second>
    pub compute_units_per_second: Option<u64>,
}

impl fmt::Display for RpcEndpointConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let Self { retries, retry_backoff, compute_units_per_second } = self;

        if let Some(retries) = retries {
            write!(f, ", retries={retries}")?;
        }

        if let Some(retry_backoff) = retry_backoff {
            write!(f, ", retry_backoff={retry_backoff}")?;
        }

        if let Some(compute_units_per_second) = compute_units_per_second {
            write!(f, ", compute_units_per_second={compute_units_per_second}")?;
        }

        Ok(())
    }
}

/// Rpc endpoint configuration variant
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RpcEndpoint {
    /// endpoint url or env
    pub endpoint: RpcEndpointUrl,

    /// Token to be used as authentication
    pub auth: Option<RpcAuth>,

    /// additional configuration
    pub config: RpcEndpointConfig,
}

impl RpcEndpoint {
    pub fn new(endpoint: RpcEndpointUrl) -> Self {
        Self { endpoint, ..Default::default() }
    }
}

impl fmt::Display for RpcEndpoint {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let Self { endpoint, auth, config } = self;
        write!(f, "{endpoint}")?;
        write!(f, "{config}")?;
        if let Some(auth) = auth {
            write!(f, ", auth={auth}")?;
        }
        Ok(())
    }
}

impl From<RpcEndpoint> for RpcEndpointType {
    fn from(config: RpcEndpoint) -> Self {
        Self::Config(config)
    }
}

impl Default for RpcEndpoint {
    fn default() -> Self {
        Self {
            endpoint: RpcEndpointUrl::Url("http://localhost:8545".to_string()),
            config: RpcEndpointConfig::default(),
            auth: None,
        }
    }
}
