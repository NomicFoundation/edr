use alloy_rlp::RlpEncodable;
use edr_evm::{chain_spec::ChainSpec, RemoteBlockConversionError};
use edr_rpc_eth::spec::RpcSpec;
use revm::optimism::{OptimismHaltReason, OptimismSpecId};
use serde::{de::DeserializeOwned, Serialize};

use crate::{rpc, transaction};

/// Chain specification for the Ethereum JSON-RPC API.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, RlpEncodable)]
pub struct OptimismChainSpec;

impl RpcSpec for OptimismChainSpec {
    type RpcBlock<Data> = edr_rpc_eth::Block<Data> where Data: Default + DeserializeOwned + Serialize;
    type RpcTransaction = crate::rpc::Transaction;
}

impl revm::primitives::ChainSpec for OptimismChainSpec {
    type Block = revm::primitives::BlockEnv;

    type Transaction = transaction::Signed;

    type Hardfork = OptimismSpecId;

    type HaltReason = OptimismHaltReason;
}

impl ChainSpec for OptimismChainSpec {
    type RpcBlockConversionError = RemoteBlockConversionError<Self>;

    type RpcTransactionConversionError = rpc::ConversionError;
}
