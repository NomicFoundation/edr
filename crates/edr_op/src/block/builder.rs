use edr_eth::{
    block::PartialHeader, spec::EthHeaderConstants, trie::KECCAK_NULL_RLP, Address, Bytes, HashMap,
};
use edr_evm::{
    blockchain::SyncBlockchain,
    config::CfgEnv,
    inspector::Inspector,
    precompile::PrecompileFn,
    spec::ContextForChainSpec,
    state::{DatabaseComponents, SyncState, WrapDatabaseRef},
    BlockBuilder, BlockBuilderCreationError, BlockInputs, BlockTransactionErrorForChainSpec,
    EthBlockBuilder, MineBlockResultAndState,
};
use op_revm::{L1BlockInfo, OpHaltReason};

use crate::{
    block::LocalBlock, predeploys::L2_TO_L1_MESSAGE_PASSER_ADDRESS, receipt::BlockReceiptFactory,
    transaction, OpChainSpec, OpSpecId,
};

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
        mut inputs: BlockInputs,
        mut overrides: edr_eth::block::HeaderOverrides,
    ) -> Result<Self, BlockBuilderCreationError<Self::BlockchainError, OpSpecId, Self::StateError>>
    {
        // TODO: https://github.com/NomicFoundation/edr/issues/990
        // Replace this once we can detect chain-specific block inputs in the provider
        // and avoid passing them as input.
        if cfg.spec >= OpSpecId::CANYON {
            // `EthBlockBuilder` expects `inputs.withdrawals.is_some()` despite OP not
            // supporting withdrawals.
            inputs.withdrawals = Some(Vec::new());
        }

        if cfg.spec >= OpSpecId::ISTHMUS {
            let withdrawals_root = overrides
                .withdrawals_root
                .map_or_else(
                    || {
                        let storage_root =
                            state.account_storage_root(&L2_TO_L1_MESSAGE_PASSER_ADDRESS)?;

                        Ok(storage_root.unwrap_or(KECCAK_NULL_RLP))
                    },
                    Ok,
                )
                .map_err(BlockBuilderCreationError::State)?;

            overrides.withdrawals_root = Some(withdrawals_root);
        }

        if cfg.spec >= OpSpecId::HOLOCENE {
            const DYNAMIC_BASE_FEE_PARAM_VERSION: u8 = 0x0;

            overrides.extra_data = Some(overrides.extra_data.unwrap_or_else(|| {
                // Ensure that the same base fee parameters are used in the EthBlockBuilder
                // and in the extra data.
                let base_fee_params = overrides.base_fee_params.get_or_insert_with(|| {
                    *OpChainSpec::BASE_FEE_PARAMS
                        .at_hardfork(cfg.spec)
                        .expect("Chain spec must have base fee params for post-London hardforks")
                });

                let denominator: [u8; 4] = u32::try_from(base_fee_params.max_change_denominator)
                    .expect("Base fee denominators can only be up to u32::MAX")
                    .to_be_bytes();
                let elasticity: [u8; 4] = u32::try_from(base_fee_params.elasticity_multiplier)
                    .expect("Base fee elasticity can only be up to u32::MAX")
                    .to_be_bytes();

                let bytes: Box<[u8]> = Box::new([
                    DYNAMIC_BASE_FEE_PARAM_VERSION,
                    denominator[0],
                    denominator[1],
                    denominator[2],
                    denominator[3],
                    elasticity[0],
                    elasticity[1],
                    elasticity[2],
                    elasticity[3],
                ]);

                Bytes::from(bytes)
            }));
        }

        let eth = EthBlockBuilder::new(blockchain, state, cfg, inputs, overrides)?;

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
