use edr_eth::{block::PartialHeader, Address, HashMap};
use edr_evm::{
    blockchain::SyncBlockchain,
    config::CfgEnv,
    inspector::Inspector,
    precompile::PrecompileFn,
    spec::ContextForChainSpec,
    state::{DatabaseComponents, SyncState, WrapDatabaseRef},
    BlockBuilder, BlockTransactionErrorForChainSpec, EthBlockBuilder, MineBlockResultAndState,
};
use op_revm::{L1BlockInfo, OpHaltReason};

use crate::{block::LocalBlock, receipt::BlockReceiptFactory, transaction, OpChainSpec, OpSpecId};

/// Block builder for OP.
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
        cfg: CfgEnv<OpSpecId>,
        options: edr_eth::block::BlockOptions,
    ) -> Result<
        Self,
        edr_evm::BlockBuilderCreationError<Self::BlockchainError, OpSpecId, Self::StateError>,
    > {
        let eth = EthBlockBuilder::new(blockchain, state, cfg, options)?;

        let l1_block_info = {
            let mut db = WrapDatabaseRef(DatabaseComponents {
                blockchain: eth.blockchain(),
                state: eth.state(),
            });

            let l2_block_number = eth.header().number;
            op_revm::L1BlockInfo::try_fetch(&mut db, l2_block_number, eth.config().spec)?
        };

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
        custom_precompiles: &HashMap<Address, PrecompileFn>,
    ) -> Result<
        (),
        BlockTransactionErrorForChainSpec<Self::BlockchainError, OpChainSpec, Self::StateError>,
    > {
        self.eth.add_transaction(transaction, custom_precompiles)
    }

    fn add_transaction_with_inspector<InspectorT>(
        &mut self,
        transaction: transaction::Signed,
        inspector: &mut InspectorT,
        custom_precompiles: &HashMap<Address, PrecompileFn>,
    ) -> Result<
        (),
        BlockTransactionErrorForChainSpec<Self::BlockchainError, OpChainSpec, Self::StateError>,
    >
    where
        InspectorT: for<'inspector> Inspector<
            ContextForChainSpec<
                OpChainSpec,
                WrapDatabaseRef<
                    DatabaseComponents<
                        &'inspector dyn SyncBlockchain<
                            OpChainSpec,
                            Self::BlockchainError,
                            Self::StateError,
                        >,
                        &'inspector dyn SyncState<Self::StateError>,
                    >,
                >,
            >,
        >,
    {
        self.eth
            .add_transaction_with_inspector(transaction, inspector, custom_precompiles)
    }

    fn finalize(
        self,
        rewards: Vec<(Address, u128)>,
    ) -> Result<MineBlockResultAndState<OpHaltReason, LocalBlock, Self::StateError>, Self::StateError>
    {
        let receipt_factory = self.block_receipt_factory();
        self.eth.finalize(&receipt_factory, rewards)
    }
}
