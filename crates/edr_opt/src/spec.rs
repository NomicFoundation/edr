use edr_rpc_eth::spec::RpcSpec;
use serde::{de::DeserializeOwned, Serialize};

/// Chain specification for the Ethereum JSON-RPC API.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct OptimismChainSpec;

impl RpcSpec for OptimismChainSpec {
    type RpcBlock<Data> = edr_rpc_eth::Block<Data> where Data: Default + DeserializeOwned + Serialize;
    type RpcTransaction = crate::rpc::Transaction;
}
