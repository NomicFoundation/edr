use core::fmt::Debug;

use edr_eth::block::PartialHeader;
use edr_evm::{
    state::{DatabaseComponents, WrapDatabaseRef},
    BlockBuilder, BlockBuilderAndError, EthBlockBuilder, MineBlockResultAndState,
};
use revm_optimism::{L1BlockInfo, OptimismHaltReason, OptimismSpecId};

use crate::{block::LocalBlock, receipt::BlockReceiptFactory, transaction, OptimismChainSpec};

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

        Ok(Self { eth, l1_block_info })
    }

    fn block_receipt_factory(&self) -> BlockReceiptFactory {
        let l1_block_info = op_alloy_rpc_types::receipt::L1BlockInfo {
            l1_gas_price: Some(
                self.l1_block_info
                    .l1_base_fee
                    .try_into()
                    .expect("L1 gas price cannot be larger than u128::max"),
            ),
            // Not supported, as it was deprecated post-Fjord
            l1_gas_used: None,
            l1_fee: Some(
                self.l1_block_info
                    .l1_base_fee
                    .try_into()
                    .expect("L1 fee cannot be larger than u128::max"),
            ),
            // Not supported, as it was deprecated post-Ecotone
            l1_fee_scalar: None,
            l1_base_fee_scalar: Some(
                self.l1_block_info
                    .l1_base_fee_scalar
                    .try_into()
                    .expect("L1 base fee scalar cannot be larger than u128::max"),
            ),
            l1_blob_base_fee: self.l1_block_info.l1_blob_base_fee.map(|scalar| {
                scalar
                    .try_into()
                    .expect("L1 blob base fee cannot be larger than u128::max")
            }),
            l1_blob_base_fee_scalar: self.l1_block_info.l1_blob_base_fee_scalar.map(|scalar| {
                scalar
                    .try_into()
                    .expect("L1 blob base fee scalar cannot be larger than u128::max")
            }),
        };

        BlockReceiptFactory { l1_block_info }
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
        let receipt_factory = self.block_receipt_factory();
        self.eth.finalize(&receipt_factory, rewards)
    }
}
