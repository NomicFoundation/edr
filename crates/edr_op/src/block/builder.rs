use edr_block_header::{HeaderOverrides, PartialHeader};
use edr_database_components::DatabaseComponents;
use edr_eip1559::ConstantBaseFeeParams;
use edr_evm::{
    blockchain::SyncBlockchainForChainSpec,
    config::CfgEnv,
    inspector::Inspector,
    precompile::PrecompileFn,
    spec::{base_fee_params_for, ContextForChainSpec},
    state::WrapDatabaseRef,
    BlockBuilder, BlockBuilderCreationError, BlockInputs, BlockTransactionErrorForChainSpec,
    EthBlockBuilder, MineBlockResultAndState,
};
use edr_primitives::{Address, Bytes, HashMap, B256, KECCAK_NULL_RLP, U256};
use edr_state_api::SyncState;
use op_revm::{L1BlockInfo, OpHaltReason, OpSpecId};

use crate::{
    block::LocalBlock,
    eip1559::{encode_dynamic_base_fee_params, DYNAMIC_BASE_FEE_PARAM_VERSION},
    predeploys::L2_TO_L1_MESSAGE_PASSER_ADDRESS,
    receipt::BlockReceiptFactory,
    spec::op_base_fee_params_overrides,
    transaction, Hardfork, OpChainSpec,
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
        blockchain: &'builder dyn SyncBlockchainForChainSpec<
            Self::BlockchainError,
            OpChainSpec,
            Self::StateError,
        >,
        state: Box<dyn SyncState<Self::StateError>>,
        cfg: CfgEnv<Hardfork>,
        mut inputs: BlockInputs,
        mut overrides: HeaderOverrides<Hardfork>,
        custom_precompiles: &'builder HashMap<Address, PrecompileFn>,
    ) -> Result<Self, BlockBuilderCreationError<Self::BlockchainError, Hardfork, Self::StateError>>
    {
        // TODO: https://github.com/NomicFoundation/edr/issues/990
        // Replace this once we can detect chain-specific block inputs in the provider
        // and avoid passing them as input.
        if cfg.spec >= Hardfork::CANYON {
            // `EthBlockBuilder` expects `inputs.withdrawals.is_some()` despite OP not
            // supporting withdrawals.
            inputs.withdrawals = Some(Vec::new());
        }

        overrides.withdrawals_root = overrides
            .withdrawals_root
            .map_or_else(
                || define_op_withdrawals_root(cfg.spec, &state),
                |value| Ok(Some(value)),
            )
            .map_err(BlockBuilderCreationError::State)?;

        if cfg.spec >= Hardfork::HOLOCENE {
            // For post-Holocene blocks, store the encoded base fee parameters to be used in
            // the next block as `extraData`. See: <https://specs.optimism.io/protocol/holocene/exec-engine.html>
            overrides.extra_data = Some(overrides.extra_data.unwrap_or_else(|| {
                let chain_base_fee_params =
                    overrides.base_fee_params.clone().unwrap_or_else(|| {
                        base_fee_params_for::<OpChainSpec>(blockchain.chain_id()).clone()
                    });

                let current_block_number = blockchain.last_block_number() + 1;
                let next_block_number = current_block_number + 1;

                let extra_data_base_fee_params = chain_base_fee_params
                    .at_condition(cfg.spec, next_block_number)
                    .expect("Chain spec must have base fee params for post-London hardforks");
                encode_dynamic_base_fee_params(extra_data_base_fee_params)
            }));

            overrides.base_fee_params = {
                let parent_block_number = blockchain.last_block_number();
                let parent_hardfork = blockchain
                    .spec_at_block_number(parent_block_number)
                    .map_err(BlockBuilderCreationError::Blockchain)?;
                let parent_block = blockchain
                    .last_block()
                    .map_err(BlockBuilderCreationError::Blockchain)?;

                op_base_fee_params_overrides(
                    parent_block.header(),
                    parent_hardfork,
                    overrides.base_fee_params,
                )
            }
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
            op_revm::L1BlockInfo::try_fetch(
                &mut db,
                U256::from(l2_block_number),
                eth.config().spec,
            )?
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
                        &'inspector dyn SyncBlockchainForChainSpec<
                            Self::BlockchainError,
                            OpChainSpec,
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

/// Prior to isthmus activation: the L2 block header's withdrawalsRoot field
/// must be:
///    - nil if Canyon has not been activated.
///    - `keccak256(rlp(empty_string_code))` if Canyon has been activated.
///
/// After Isthmus activation, the withdrawalsRoot field be the
/// `L2ToL1MessagePasser` account storage root
fn define_op_withdrawals_root<StateErrorT>(
    hardfork: OpSpecId,
    state: &dyn SyncState<StateErrorT>,
) -> Result<Option<B256>, StateErrorT> {
    if hardfork < OpSpecId::CANYON {
        Ok(None)
    } else if hardfork < OpSpecId::ISTHMUS {
        Ok(Some(KECCAK_NULL_RLP))
    } else {
        let storage_root = state.account_storage_root(&L2_TO_L1_MESSAGE_PASSER_ADDRESS)?;
        Ok(storage_root)
    }
}
/// Decodes the base fee params from Bytes considering op-stack extra-param spec
pub fn decode_base_params(extra_data: &Bytes) -> ConstantBaseFeeParams {
    let version = *extra_data
        .first()
        .expect("Extra data should have at least 1 byte for version");
    match version {
        DYNAMIC_BASE_FEE_PARAM_VERSION => {
            let denominator_bytes: [u8; 4] = extra_data
                .get(1..=4)
                .expect("Extra data should have at least 9 bytes for dynamic base fee params")
                .try_into()
                .expect("The slice should be exactly 4 bytes");

            let elasticity_bytes: [u8; 4] = extra_data
                .get(5..=8)
                .expect("Extra data should have at least 9 bytes for dynamic base fee params")
                .try_into()
                .expect("The slice should be exactly 4 bytes");

                let max_change_denominator = u32::from_be_bytes(denominator_bytes).into();
                let elasticity_multiplier = u32::from_be_bytes(elasticity_bytes).into();
                ConstantBaseFeeParams{max_change_denominator, elasticity_multiplier}
        }
        _ => panic!(
            "Unsupported base fee params version: {version}. Expected {DYNAMIC_BASE_FEE_PARAM_VERSION}."
        )
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use edr_block_header::BlockConfig;
    use edr_blockchain_api::Blockchain as _;
    use edr_evm::{blockchain::LocalBlockchain, spec::GenesisBlockFactory, GenesisBlockOptions};
    use edr_state_api::{StateDiff, SyncState};
    use op_revm::OpSpecId;

    use crate::{block::builder::define_op_withdrawals_root, hardfork::op, OpChainSpec};

    fn create_local_blockchain(hardfork: OpSpecId) -> anyhow::Result<LocalBlockchain<OpChainSpec>> {
        let genesis_block = OpChainSpec::genesis_block(
            StateDiff::default(),
            BlockConfig {
                hardfork,
                base_fee_params: &op::MAINNET_BASE_FEE_PARAMS,
            },
            GenesisBlockOptions::default(),
        )?;

       Ok(LocalBlockchain::<OpChainSpec>::new(genesis_block, StateDiff::default(), 1234, hardfork)?)
    }

    #[test]
    fn should_return_none_if_before_canyon() -> anyhow::Result<()> {
        let hardfork = OpSpecId::BEDROCK;
        let blockchain = create_local_blockchain(hardfork)?;
        let state = blockchain.state_at_block_number(0, &BTreeMap::new())?;
        let response = define_op_withdrawals_root(hardfork, &state)?;
        assert_eq!(None, response);
        Ok(())
    }
}
