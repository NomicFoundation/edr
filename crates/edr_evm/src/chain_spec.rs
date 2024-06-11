use std::fmt::Debug;

use alloy_rlp::RlpEncodable;
use edr_eth::transaction::SignedTransaction;
use edr_rpc_eth::spec::{EthRpcSpec, RpcSpec};
use revm::primitives::TxEnv;
use serde::{de::DeserializeOwned, Serialize};

/// A trait for defining a chain's associated types.
pub trait ChainSpec: Debug + alloy_rlp::Encodable + RpcSpec + 'static {
    /// The type of signed transactions used by this chain.
    type SignedTransaction: alloy_rlp::Encodable
        + Clone
        + Debug
        + TryInto<TxEnv>
        + PartialEq
        + Eq
        + SignedTransaction;
}

/// The chain specification for Ethereum Layer 1.
#[derive(Clone, Copy, Debug, PartialEq, Eq, RlpEncodable)]
pub struct L1ChainSpec;

impl ChainSpec for L1ChainSpec {
    type SignedTransaction = edr_eth::transaction::Signed;
}

impl RpcSpec for L1ChainSpec {
    type RpcBlock<Data> = <EthRpcSpec as RpcSpec>::RpcBlock<Data> where Data: Default + DeserializeOwned + Serialize;
    type RpcTransaction = <EthRpcSpec as RpcSpec>::RpcTransaction;
}
