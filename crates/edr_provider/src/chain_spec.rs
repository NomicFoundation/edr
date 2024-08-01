use edr_rpc_eth::RpcTypeFrom;

use crate::data::TransactionAndBlock;

pub trait ChainSpec:
    edr_evm::chain_spec::ChainSpec<
    RpcTransaction: RpcTypeFrom<TransactionAndBlock<Self>, Hardfork = Self::Hardfork>,
>
{
}
