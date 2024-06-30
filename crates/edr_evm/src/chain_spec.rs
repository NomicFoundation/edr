use std::fmt::Debug;

use alloy_rlp::RlpEncodable;
use edr_eth::{
    transaction::{self, SignedTransaction},
    B256,
};
use edr_rpc_eth::{
    spec::{EthRpcSpec, RpcSpec},
    TransactionConversionError,
};
use serde::{de::DeserializeOwned, Serialize};

use crate::{
    hardfork::{self, Activations},
    transaction::remote::EthRpcTransaction,
    EthBlockData, EthRpcBlock, RemoteBlockConversionError,
};

/// A trait for defining a chain's associated types.
// Bug: https://github.com/rust-lang/rust-clippy/issues/12927
#[allow(clippy::trait_duplication_in_bounds)]
pub trait ChainSpec:
    alloy_rlp::Encodable
    + revm::primitives::ChainSpec<
        Transaction: alloy_rlp::Encodable
                         + Clone
                         + Debug
                         + PartialEq
                         + Eq
                         + SignedTransaction
                         + TryFrom<
            <Self as RpcSpec>::RpcTransaction,
            Error = Self::RpcTransactionConversionError,
        >,
    > + RpcSpec<
        RpcBlock<<Self as RpcSpec>::RpcTransaction>: EthRpcBlock
                                                         + TryInto<
            EthBlockData<Self>,
            Error = Self::RpcBlockConversionError,
        >,
        RpcTransaction: EthRpcTransaction,
    > + RpcSpec<RpcBlock<B256>: EthRpcBlock>
{
    /// Type representing an error that occurs when converting an RPC block.
    type RpcBlockConversionError: Debug + std::error::Error;

    /// Type representing an error that occurs when converting an RPC
    /// transaction.
    type RpcTransactionConversionError: Debug + std::error::Error;

    /// Returns the hardfork activations corresponding to the provided chain ID,
    /// if it is associated with this chain specification.
    fn chain_hardfork_activations(chain_id: u64) -> Option<&'static Activations<Self>>;

    /// Returns the name corresponding to the provided chain ID, if it is
    /// associated with this chain specification.
    fn chain_name(chain_id: u64) -> Option<&'static str>;
}

/// A supertrait for [`ChainSpec`] that is safe to send between threads.
pub trait SyncChainSpec: ChainSpec<Transaction: Send + Sync> + Send + Sync + 'static {}

impl<ChainSpecT> SyncChainSpec for ChainSpecT where
    ChainSpecT: ChainSpec<Transaction: Send + Sync> + Send + Sync + 'static
{
}

/// The chain specification for Ethereum Layer 1.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, RlpEncodable)]
pub struct L1ChainSpec;

impl revm::primitives::ChainSpec for L1ChainSpec {
    type Block = revm::primitives::BlockEnv;

    type Hardfork = revm::primitives::SpecId;

    type HaltReason = revm::primitives::HaltReason;

    type Transaction = transaction::Signed;
}

impl ChainSpec for L1ChainSpec {
    type RpcBlockConversionError = RemoteBlockConversionError<Self>;
    type RpcTransactionConversionError = TransactionConversionError;

    fn chain_hardfork_activations(chain_id: u64) -> Option<&'static Activations<Self>> {
        hardfork::l1::chain_hardfork_activations(chain_id)
    }

    fn chain_name(chain_id: u64) -> Option<&'static str> {
        hardfork::l1::chain_name(chain_id)
    }
}

impl RpcSpec for L1ChainSpec {
    type RpcBlock<Data> = <EthRpcSpec as RpcSpec>::RpcBlock<Data> where Data: Default + DeserializeOwned + Serialize;
    type RpcTransaction = <EthRpcSpec as RpcSpec>::RpcTransaction;
}
