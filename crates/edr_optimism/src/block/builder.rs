use core::fmt::Debug;

use edr_eth::{block::PartialHeader, log::FilterLog, receipt::TransactionReceipt, B256};
use edr_evm::{
    receipt::ReceiptFactory,
    state::{DatabaseComponents, WrapDatabaseRef},
    BlockBuilder, BlockBuilderAndError, EthBlockBuilder, EthBlockReceiptFactory,
    MineBlockResultAndState,
};
use revm_optimism::{OptimismHaltReason, OptimismSpecId};

use super::LocalBlock;
use crate::{eip2718::TypedEnvelope, receipt, transaction, L1BlockInfo, OptimismChainSpec};

/// Block builder for Optimism.
pub struct Builder<'blockchain, BlockchainErrorT, DebugDataT, StateErrorT> {
    eth: EthBlockBuilder<'blockchain, BlockchainErrorT, OptimismChainSpec, DebugDataT, StateErrorT>,
    l1_block_info: L1BlockInfo,
}

impl<'blockchain, BlockchainErrorT, DebugDataT, StateErrorT: Debug + Send>
    BlockBuilder<'blockchain, OptimismChainSpec, DebugDataT>
    for Builder<'blockchain, BlockchainErrorT, DebugDataT, StateErrorT>
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
        hardfork: OptimismSpecId,
        cfg: edr_evm::config::CfgEnv,
        options: edr_eth::block::BlockOptions,
        debug_context: Option<
            edr_evm::DebugContext<
                'blockchain,
                OptimismChainSpec,
                Self::BlockchainError,
                DebugDataT,
                Box<dyn edr_evm::state::SyncState<Self::StateError>>,
            >,
        >,
    ) -> Result<
        Self,
        edr_evm::BlockBuilderCreationError<Self::BlockchainError, OptimismSpecId, Self::StateError>,
    > {
        let mut db = WrapDatabaseRef(DatabaseComponents { blockchain, state });
        let l1_block_info = revm_optimism::L1BlockInfo::try_fetch(&mut db, hardfork)?;
        let DatabaseComponents { blockchain, state } = db.0;

        let eth = EthBlockBuilder::new(blockchain, state, hardfork, cfg, options, debug_context)?;

        Ok(Self {
            eth,
            l1_block_info: l1_block_info.into(),
        })
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
        let Self { eth, l1_block_info } = self;

        match eth.add_transaction(transaction) {
            Ok(eth) => Ok(Self { eth, l1_block_info }),
            Err(BlockBuilderAndError {
                block_builder,
                error,
            }) => Err(BlockBuilderAndError {
                block_builder: Self {
                    eth: block_builder,
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
        let receipt_factory = BlockReceiptFactory::default();

        let MineBlockResultAndState {
            block: l1,
            state,
            state_diff,
            transaction_results,
        } = self.eth.finalize(&receipt_factory, rewards)?;

        Ok(MineBlockResultAndState {
            block: LocalBlock {
                eth: l1,
                l1_block_info: self.l1_block_info,
            },
            state,
            state_diff,
            transaction_results,
        })
    }
}

/// Block receipt factory for Optimism.
#[derive(Default)]
pub struct BlockReceiptFactory {
    l1_block_info: L1BlockInfo,
}

impl ReceiptFactory<TypedEnvelope<receipt::Execution<FilterLog>>> for BlockReceiptFactory {
    type Output = receipt::Block;

    fn create_receipt(
        &self,
        transaction_receipt: TransactionReceipt<TypedEnvelope<receipt::Execution<FilterLog>>>,
        block_hash: &B256,
        block_number: u64,
    ) -> Self::Output {
        let l1_block_info = if matches!(transaction_receipt.inner, TypedEnvelope::Deposit(_)) {
            None
        } else {
            Some(self.l1_block_info.clone())
        };

        let eth = {
            let receipt_factory = EthBlockReceiptFactory::default();
            receipt_factory.create_receipt(transaction_receipt, block_hash, block_number)
        };

        receipt::Block { eth, l1_block_info }
    }
}
