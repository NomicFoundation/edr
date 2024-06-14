use std::fmt::Debug;

use alloy_rlp::RlpEncodable;
use edr_eth::{transaction::SignedTransaction, B256};
use edr_rpc_eth::spec::{EthRpcSpec, RpcSpec};
use revm::primitives::TxEnv;
use serde::{de::DeserializeOwned, Serialize};

use crate::{transaction::remote::EthRpcTransaction, EthRpcBlock, IntoRemoteBlock};

/// A trait for defining a chain's associated types.
// Bug: https://github.com/rust-lang/rust-clippy/issues/12927
#[allow(clippy::trait_duplication_in_bounds)]
pub trait ChainSpec:
    Debug
    + alloy_rlp::Encodable
    + RpcSpec<
        RpcBlock<<Self as RpcSpec>::RpcTransaction>: EthRpcBlock + IntoRemoteBlock<Self>,
        RpcTransaction: EthRpcTransaction,
    > + RpcSpec<RpcBlock<B256>: EthRpcBlock>
{
    /// The type of signed transactions used by this chain.
    type SignedTransaction: alloy_rlp::Encodable
        + Clone
        + Debug
        + TryInto<TxEnv>
        + PartialEq
        + Eq
        + SignedTransaction;
}

/// A supertrait for [`ChainSpec`] that is safe to send between threads.
pub trait SyncChainSpec: ChainSpec<SignedTransaction: Send + Sync> + Send + Sync + 'static {}

impl<ChainSpecT> SyncChainSpec for ChainSpecT where
    ChainSpecT: ChainSpec<SignedTransaction: Send + Sync> + Send + Sync + 'static
{
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
