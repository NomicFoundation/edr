use edr_eth::{block::PartialHeader, Address};
use edr_evm::{
    config::CfgEnv,
    state::{DatabaseComponents, WrapDatabaseRef},
    BlockBuilder, BlockTransactionError, EthBlockBuilder, MineBlockResultAndState,
};
use revm_optimism::{L1BlockInfo, OptimismHaltReason};

use crate::{block::LocalBlock, receipt::BlockReceiptFactory, transaction, OpChainSpec, OpSpec};

/// Block builder for Optimism.
pub struct Builder<'blockchain, BlockchainErrorT, StateErrorT> {
    eth: EthBlockBuilder<'blockchain, BlockchainErrorT, OpChainSpec, StateErrorT>,
    l1_block_info: L1BlockInfo,
}

impl<'blockchain, BlockchainErrorT, StateErrorT> BlockBuilder<'blockchain, OpChainSpec>
    for Builder<'blockchain, BlockchainErrorT, StateErrorT>
where
    BlockchainErrorT: Send + std::error::Error,
    StateErrorT: Send + std::error::Error,
{
    type BlockchainError = BlockchainErrorT;
    type StateError = StateErrorT;

    fn new_block_builder(
        blockchain: &'blockchain dyn edr_evm::blockchain::SyncBlockchain<
            OpChainSpec,
            Self::BlockchainError,
            Self::StateError,
        >,
        state: Box<dyn edr_evm::state::SyncState<Self::StateError>>,
        cfg: CfgEnv<OpSpec>,
        options: edr_eth::block::BlockOptions,
    ) -> Result<
        Self,
        edr_evm::BlockBuilderCreationError<Self::BlockchainError, OpSpec, Self::StateError>,
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
        &mut self,
        transaction: transaction::Signed,
    ) -> Result<(), BlockTransactionError<Self::BlockchainError, OpChainSpec, Self::StateError>>
    {
        self.eth.add_transaction(transaction)
    }

    fn add_transaction_with_inspector<'context, 'extension, ExtensionT, FrameT>(
        &mut self,
        transaction: transaction::Signed,
        extension: &'extension mut edr_evm::ContextExtension<ExtensionT, FrameT>,
    ) -> Result<(), BlockTransactionError<Self::BlockchainError, OpChainSpec, Self::StateError>>
    where
        'blockchain: 'context,
        'extension: 'context,
        OpChainSpec: 'context,
        FrameT: edr_evm::evm::Frame<
            Context<'context> = edr_evm::extension::ExtendedContext<
                'context,
                edr_evm::spec::ContextForChainSpec<
                    OpChainSpec,
                    WrapDatabaseRef<
                        DatabaseComponents<
                            &'context dyn edr_evm::blockchain::SyncBlockchain<
                                OpChainSpec,
                                Self::BlockchainError,
                                Self::StateError,
                            >,
                            &'context dyn edr_evm::state::SyncState<Self::StateError>,
                        >,
                    >,
                >,
                ExtensionT,
            >,
            Error = edr_evm::transaction::TransactionError<
                Self::BlockchainError,
                OpChainSpec,
                Self::StateError,
            >,
            FrameInit = edr_evm::interpreter::FrameInput,
            FrameResult = edr_evm::evm::FrameResult,
        >,
        Self::BlockchainError: 'context,
        Self::StateError: 'context,
    {
        self.eth
            .add_transaction_with_inspector(transaction, extension)
    }

    fn finalize(
        self,
        rewards: Vec<(Address, u128)>,
    ) -> Result<
        MineBlockResultAndState<OptimismHaltReason, LocalBlock, Self::StateError>,
        Self::StateError,
    > {
        let receipt_factory = self.block_receipt_factory();
        self.eth.finalize(&receipt_factory, rewards)
    }
}
