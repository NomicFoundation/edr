use edr_eth::{
    block::PartialHeader, eips::eip1559::ConstantBaseFeeParams, spec::EthHeaderConstants,
    trie::KECCAK_NULL_RLP, Address, Bytes, HashMap,
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
pub struct Builder<'builder, BlockchainErrorT, StateErrorT> {
    eth: EthBlockBuilder<'builder, BlockchainErrorT, OpChainSpec, StateErrorT>,
    l1_block_info: L1BlockInfo,
}

impl<'builder, BlockchainErrorT, StateErrorT> BlockBuilder<'builder, OpChainSpec>
    for Builder<'builder, BlockchainErrorT, StateErrorT>
where
    BlockchainErrorT: Send + std::error::Error,
    StateErrorT: Send + std::error::Error,
{
    type BlockchainError = BlockchainErrorT;
    type StateError = StateErrorT;

    fn new_block_builder(
        blockchain: &'builder dyn edr_evm::blockchain::SyncBlockchain<
            OpChainSpec,
            Self::BlockchainError,
            Self::StateError,
        >,
        state: Box<dyn edr_evm::state::SyncState<Self::StateError>>,
        cfg: CfgEnv<OpSpecId>,
        mut inputs: BlockInputs,
        mut overrides: edr_eth::block::HeaderOverrides,
        custom_precompiles: &'builder HashMap<Address, PrecompileFn>,
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

            let base_fee_params = overrides.base_fee_params.map_or_else(|| -> Result<ConstantBaseFeeParams, BlockBuilderCreationError<Self::BlockchainError, OpSpecId, Self::StateError>> {
                let parent_block_number = blockchain.last_block_number();
                let parent_hardfork = blockchain
                    .spec_at_block_number(parent_block_number)
                    .map_err(BlockBuilderCreationError::Blockchain)?;

                if parent_hardfork >= OpSpecId::HOLOCENE {
                    // Take parameters from parent block's extra data
                    let parent_block = blockchain
                        .last_block()
                        .map_err(BlockBuilderCreationError::Blockchain)?;

                    let parent_header = parent_block.header();
                    let extra_data = &parent_header.extra_data;

                    let version = *extra_data.first()
                        .expect("Extra data should have at least 1 byte for version");

                    let base_fee_params = match version {
                        DYNAMIC_BASE_FEE_PARAM_VERSION => {
                            let denominator_bytes: [u8; 4] = extra_data[1..=4]
                                .try_into()
                                .expect("The slice should be exactly 4 bytes");

                            let elasticity_bytes: [u8; 4] = extra_data[5..=8]
                                .try_into()
                                .expect("The slice should be exactly 4 bytes");

                            ConstantBaseFeeParams {
                                max_change_denominator: u32::from_be_bytes(denominator_bytes)
                                    .into(),
                                elasticity_multiplier: u32::from_be_bytes(elasticity_bytes).into(),
                            }
                        }
                        _ => panic!(
                            "Unsupported base fee params version: {version}. Expected {DYNAMIC_BASE_FEE_PARAM_VERSION}."
                        )
                    };

                    Ok(base_fee_params)
                } else {
                    // Use the prior EIP-1559 constants.
                    let base_fee_params = *OpChainSpec::BASE_FEE_PARAMS
                        .at_hardfork(cfg.spec)
                        .expect("Chain spec must have base fee params for post-London hardforks");

                    Ok(base_fee_params)
                }
            }, Ok)?;

            let extra_data = overrides.extra_data.unwrap_or_else(|| {
                let denominator: [u8; 4] = u32::try_from(base_fee_params.max_change_denominator)
                    .expect("Base fee denominators can only be up to u32::MAX")
                    .to_be_bytes();
                let elasticity: [u8; 4] = u32::try_from(base_fee_params.elasticity_multiplier)
                    .expect("Base fee elasticity can only be up to u32::MAX")
                    .to_be_bytes();

                let mut extra_data = [0u8; 9];
                extra_data[0] = DYNAMIC_BASE_FEE_PARAM_VERSION;
                extra_data[1..=4].copy_from_slice(&denominator);
                extra_data[5..=8].copy_from_slice(&elasticity);

                let bytes: Box<[u8]> = Box::new(extra_data);
                Bytes::from(bytes)
            });

            overrides.base_fee_params = Some(base_fee_params);
            overrides.extra_data = Some(extra_data);
        }

        let eth = EthBlockBuilder::new(
            blockchain,
            state,
            cfg,
            inputs,
            overrides,
            custom_precompiles,
        )?;

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
    ) -> Result<
        (),
        BlockTransactionErrorForChainSpec<Self::BlockchainError, OpChainSpec, Self::StateError>,
    > {
        self.eth.add_transaction(transaction)
    }

    fn add_transaction_with_inspector<InspectorT>(
        &mut self,
        transaction: transaction::Signed,
        inspector: &mut InspectorT,
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
            .add_transaction_with_inspector(transaction, inspector)
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
