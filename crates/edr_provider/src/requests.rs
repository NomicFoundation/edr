pub(crate) mod debug;
/// Ethereum RPC request types
pub(crate) mod eth;
/// Hardhat RPC request types
pub(crate) mod hardhat;
mod methods;
mod serde;
/// Types and functions for validating JSON-RPC requests.
pub mod validation;

use std::{fmt, marker::PhantomData};

use ::serde::{
    de::{self, MapAccess, SeqAccess, Visitor},
    Deserialize, Deserializer, Serialize,
};
use derive_where::derive_where;
use edr_rpc_eth::spec::RpcSpec;

#[cfg(feature = "test-utils")]
pub use self::eth::resolve_estimate_gas_request;
pub use crate::requests::{
    methods::{IntervalConfig, MethodInvocation},
    serde::{InvalidRequestReason, Timestamp},
};

/// JSON-RPC request for the provider.
#[derive(Serialize)]
#[derive_where(Clone, Debug; ChainSpecT::RpcCallRequest, ChainSpecT::RpcTransactionRequest)]
#[serde(bound = "")]
pub enum ProviderRequest<ChainSpecT: RpcSpec> {
    /// A single JSON-RPC request
    Single(Box<MethodInvocation<ChainSpecT>>),
    /// A batch of requests
    Batch(Vec<MethodInvocation<ChainSpecT>>),
}

impl<ChainSpecT: RpcSpec> ProviderRequest<ChainSpecT> {
    /// Constructs a new instance from a single [`MethodInvocation`].
    pub fn with_single(method: MethodInvocation<ChainSpecT>) -> Self {
        Self::Single(Box::new(method))
    }
}

// Custom deserializer for `ProviderRequest` instead of using
// `#[serde(untagged)]` as the latter hides custom error messages which are
// important to propagate to users.
impl<'de, ChainSpecT: RpcSpec> Deserialize<'de> for ProviderRequest<ChainSpecT> {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive_where(Default)]
        struct SingleOrBatchRequestVisitor<ChainSpecT: RpcSpec> {
            phantom: PhantomData<ChainSpecT>,
        }

        impl<'de, ChainSpecT: RpcSpec> Visitor<'de> for SingleOrBatchRequestVisitor<ChainSpecT> {
            type Value = ProviderRequest<ChainSpecT>;

            fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str("single or batch request")
            }

            fn visit_seq<A>(self, seq: A) -> Result<Self::Value, A::Error>
            where
                A: SeqAccess<'de>,
            {
                // Forward to deserializer of `Vec<MethodInvocation>`
                Ok(ProviderRequest::Batch(Deserialize::deserialize(
                    de::value::SeqAccessDeserializer::new(seq),
                )?))
            }

            fn visit_map<M>(self, map: M) -> Result<ProviderRequest<ChainSpecT>, M::Error>
            where
                M: MapAccess<'de>,
            {
                // Forward to deserializer of `MethodInvocation`
                Ok(ProviderRequest::with_single(Deserialize::deserialize(
                    de::value::MapAccessDeserializer::new(map),
                )?))
            }
        }

        deserializer.deserialize_any(SingleOrBatchRequestVisitor::<ChainSpecT>::default())
    }
}
