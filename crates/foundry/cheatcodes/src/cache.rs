//! Support types for configuring storage caching

use std::{fmt, str::FromStr};

use alloy_chains::Chain;

/// Settings to configure caching of remote.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct StorageCachingConfig {
    /// Chains to cache.
    pub chains: CachedChains,
    /// Endpoints to cache.
    pub endpoints: CachedEndpoints,
}

impl StorageCachingConfig {
    /// Whether caching should be enabled for the endpoint
    pub fn enable_for_endpoint(&self, endpoint: impl AsRef<str>) -> bool {
        self.endpoints.is_match(endpoint)
    }

    /// Whether caching should be enabled for the chain id
    pub fn enable_for_chain_id(&self, chain_id: u64) -> bool {
        // ignore dev chains
        if [99, 1337, 31337].contains(&chain_id) {
            return false;
        }
        self.chains.is_match(chain_id)
    }
}

/// What chains to cache
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum CachedChains {
    /// Cache all chains
    #[default]
    All,
    /// Don't cache anything
    None,
    /// Only cache these chains
    Chains(Vec<Chain>),
}
impl CachedChains {
    /// Whether the `endpoint` matches
    pub fn is_match(&self, chain: u64) -> bool {
        match self {
            Self::All => true,
            Self::None => false,
            Self::Chains(chains) => chains.iter().any(|c| c.id() == chain),
        }
    }
}

/// What endpoints to enable caching for
#[derive(Clone, Debug, Default)]
pub enum CachedEndpoints {
    /// Cache all endpoints
    #[default]
    All,
    /// Only cache non-local host endpoints
    Remote,
    /// Only cache these chains
    Pattern(regex::Regex),
}

impl CachedEndpoints {
    /// Whether the `endpoint` matches
    pub fn is_match(&self, endpoint: impl AsRef<str>) -> bool {
        let endpoint = endpoint.as_ref();
        match self {
            Self::All => true,
            Self::Remote => !endpoint.contains("localhost:") && !endpoint.contains("127.0.0.1:"),
            Self::Pattern(re) => re.is_match(endpoint),
        }
    }
}

impl PartialEq for CachedEndpoints {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Pattern(a), Self::Pattern(b)) => a.as_str() == b.as_str(),
            (&Self::All, &Self::All) => true,
            (&Self::Remote, &Self::Remote) => true,
            _ => false,
        }
    }
}

impl Eq for CachedEndpoints {}

impl fmt::Display for CachedEndpoints {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::All => f.write_str("all"),
            Self::Remote => f.write_str("remote"),
            Self::Pattern(s) => s.fmt(f),
        }
    }
}

impl FromStr for CachedEndpoints {
    type Err = regex::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "all" => Ok(Self::All),
            "remote" => Ok(Self::Remote),
            _ => Ok(Self::Pattern(s.parse()?)),
        }
    }
}
