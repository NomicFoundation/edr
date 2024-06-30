use alloy_rlp::RlpEncodable;
use edr_evm::{chain_spec::ChainSpec, RemoteBlockConversionError};
use edr_rpc_eth::spec::RpcSpec;
use revm::optimism::{OptimismHaltReason, OptimismSpecId};
use serde::{de::DeserializeOwned, Serialize};

use crate::{hardfork, rpc, transaction};

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

    fn chain_hardfork_activations(
        chain_id: u64,
    ) -> Option<&'static edr_evm::hardfork::Activations<Self>> {
        hardfork::chain_hardfork_activations(chain_id)
    }

    fn chain_name(chain_id: u64) -> Option<&'static str> {
        hardfork::chain_name(chain_id)
    }
}
