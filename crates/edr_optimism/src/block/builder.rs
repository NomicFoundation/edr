use core::fmt::Debug;

use edr_eth::block::PartialHeader;
use edr_evm::{
    config::CfgEnv,
    state::{DatabaseComponents, WrapDatabaseRef},
    BlockBuilder, EthBlockBuilder, MineBlockResultAndState,
};
use revm_optimism::{L1BlockInfo, OptimismHaltReason};

use crate::{
    block::LocalBlock, receipt::BlockReceiptFactory, transaction, OptimismChainSpec, OptimismSpecId,
};

/// Block builder for Optimism.
pub struct Builder<'blockchain, BlockchainErrorT, StateErrorT> {
    eth: EthBlockBuilder<'blockchain, BlockchainErrorT, OptimismChainSpec, StateErrorT>,
    l1_block_info: L1BlockInfo,
}

impl<'blockchain, BlockchainErrorT, StateErrorT> BlockBuilder<'blockchain, OptimismChainSpec>
    for Builder<'blockchain, BlockchainErrorT, StateErrorT>
where
    BlockchainErrorT: Send + std::error::Error,
    StateErrorT: Send + std::error::Error,
{
    type BlockchainError = BlockchainErrorT;
    type StateError = StateErrorT;

    fn new_block_builder(
        blockchain: &'blockchain dyn edr_evm::blockchain::SyncBlockchain<
            OptimismChainSpec,
            Self::BlockchainError,
            Self::StateError,
        >,
        state: Box<dyn edr_evm::state::SyncState<Self::StateError>>,
        cfg: CfgEnv<OptimismSpecId>,
        options: edr_eth::block::BlockOptions,
    ) -> Result<
        Self,
        edr_evm::BlockBuilderCreationError<Self::BlockchainError, OptimismSpecId, Self::StateError>,
    > {
        let mut db = WrapDatabaseRef(DatabaseComponents { blockchain, state });
        let l1_block_info = revm_optimism::L1BlockInfo::try_fetch(&mut db, cfg.spec)?;
        let DatabaseComponents { blockchain, state } = db.0;

        let eth = EthBlockBuilder::new(blockchain, state, cfg, options)?;

        Ok(Self { eth, l1_block_info })
    }

    fn block_receipt_factory(&self) -> BlockReceiptFactory {
        BlockReceiptFactory {
            l1_block_info: self.l1_block_info.clone(),
        }
    }

    fn header(&self) -> &PartialHeader {
        self.eth.header()
    }

    fn add_transaction(
        self,
        transaction: transaction::Signed,
    ) -> Result<
        Self,
        edr_evm::BlockBuilderAndError<
            Self,
            edr_evm::BlockTransactionError<
                OptimismChainSpec,
                Self::BlockchainError,
                Self::StateError,
            >,
        >,
    > {
        let Self {
            eth,
            hardfork,
            l1_block_info,
        } = self;

        match eth.add_transaction(transaction) {
            Ok(eth) => Ok(Self {
                eth,
                hardfork,
                l1_block_info,
            }),
            Err(BlockBuilderAndError {
                block_builder,
                error,
            }) => Err(BlockBuilderAndError {
                block_builder: Self {
                    eth: block_builder,
                    hardfork,
                    l1_block_info,
                },
                error,
            }),
        }
    }

    fn finalize(
        self,
        rewards: Vec<(edr_eth::Address, edr_eth::U256)>,
    ) -> Result<
        MineBlockResultAndState<OptimismHaltReason, LocalBlock, Self::StateError>,
        Self::StateError,
    > {
        let receipt_factory = self.block_receipt_factory();
        self.eth.finalize(&receipt_factory, rewards)
    }
}
