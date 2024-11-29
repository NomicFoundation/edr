use edr_eth::block::PartialHeader;
use edr_evm::{
    block::EthLocalBlock as L1LocalBlock,
    state::{DatabaseComponents, WrapDatabaseRef},
    BlockBuilder, EthBlockBuilder, MineBlockResultAndState,
};
use revm_optimism::L1BlockInfo;

use crate::OptimismChainSpec;

pub struct LocalBlock<ExecutionReceiptT, SignedTransactionT> {
    l1: L1LocalBlock<ExecutionReceiptT, SignedTransactionT>,
    l1_block_info: L1BlockInfo,
}

pub struct Builder<'blockchain, BlockchainErrorT, DebugDataT, StateErrorT> {
    eth: EthBlockBuilder<'blockchain, BlockchainErrorT, OptimismChainSpec, DebugDataT, StateErrorT>,
    l1_block_info: L1BlockInfo,
}

impl<'blockchain, BlockchainErrorT, DebugDataT, StateErrorT>
    BlockBuilder<'blockchain, BlockchainErrorT, OptimismChainSpec, DebugDataT, StateErrorT>
    for Builder<'blockchain, BlockchainErrorT, DebugDataT, StateErrorT>
{
    type BlockchainError = BlockchainErrorT;
    type StateError = StateErrorT;

    fn new_block_builder(
        blockchain: &'blockchain dyn edr_evm::blockchain::SyncBlockchain<
            BlockchainErrorT,
            Self::BlockchainError,
            Self::StateError,
        >,
        state: Box<dyn edr_evm::state::SyncState<Self::StateError>>,
        hardfork: <BlockchainErrorT>::Hardfork,
        cfg: edr_evm::config::CfgEnv,
        options: edr_eth::block::BlockOptions,
        debug_context: Option<
            edr_evm::DebugContext<
                'blockchain,
                BlockchainErrorT,
                Self::BlockchainError,
                OptimismChainSpec,
                Box<dyn edr_evm::state::SyncState<Self::StateError>>,
            >,
        >,
    ) -> Result<
        Self,
        edr_evm::BlockBuilderCreationError<Self::BlockchainError, <BlockchainErrorT>::Hardfork>,
    > {
        let mut db = WrapDatabaseRef(DatabaseComponents { blockchain, state });
        let l1_block_info = L1BlockInfo::try_fetch(&mut db, hardfork)?;
        let DatabaseComponents { blockchain, state } = db.0;

        let eth = EthBlockBuilder::new_block_builder(
            blockchain,
            state,
            hardfork,
            cfg,
            options,
            debug_context,
        )?;

        Ok(Self { eth, l1_block_info })
    }

    fn header(&self) -> &PartialHeader {
        self.eth.header()
    }

    fn add_transaction(
        self,
        transaction: <BlockchainErrorT>::SignedTransaction,
    ) -> Result<
        Self,
        edr_evm::BlockBuilderAndError<
            Self,
            edr_evm::BlockTransactionError<
                BlockchainErrorT,
                Self::BlockchainError,
                Self::StateError,
            >,
        >,
    > {
        self.eth.add_transaction(transaction)
    }

    fn finalize(
        self,
        rewards: Vec<(edr_eth::Address, edr_eth::U256)>,
    ) -> Result<MineBlockResultAndState<BlockchainErrorT, Self::StateError>, Self::StateError> {
        let MineBlockResultAndState {
            block: l1,
            state,
            state_diff,
            transaction_results,
        } = self.eth.finalize(rewards)?;

        Ok(MineBlockResultAndState {
            block: LocalBlock {
                l1,
                l1_block_info: self.l1_block_info,
            },
            state,
            state_diff,
            transaction_results,
        })
    }
}
