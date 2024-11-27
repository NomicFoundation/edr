use edr_evm::EthBlockBuilder;

use crate::OptimismChainSpec;

pub struct BlockBuilder<'blockchain, BlockchainErrorT, DebugDataT, StateErrorT> {
    eth: EthBlockBuilder<'blockchain, BlockchainErrorT, OptimismChainSpec, DebugDataT, StateErrorT>,
}
