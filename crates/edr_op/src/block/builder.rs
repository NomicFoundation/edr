use core::fmt::Debug;

use edr_block_builder_api::{
    BlockBuilder, BlockBuilderCreationError, BlockInputs, BlockTransactionError,
    BuiltBlockAndState, DatabaseComponents, PrecompileFn, WrapDatabaseRef,
};
use edr_block_header::{overridden_block_number, HeaderOverrides, PartialHeader};
use edr_chain_l1::block::EthBlockBuilder;
use edr_chain_spec::TransactionValidation;
use edr_chain_spec_block::BlockChainSpec;
use edr_eip1559::ConstantBaseFeeParams;
use edr_evm_spec::{config::EvmConfig, DatabaseComponentError};
use edr_primitives::{Address, Bytes, HashMap, KECCAK_NULL_RLP, U256};
use edr_state_api::{DynState, StateError};

use crate::{
    block::LocalBlock,
    eip1559::{encode_dynamic_base_fee_params, DYNAMIC_BASE_FEE_PARAM_VERSION},
    predeploys::L2_TO_L1_MESSAGE_PASSER_ADDRESS,
    receipt::{block::OpBlockReceipt, execution::OpExecutionReceiptBuilder},
    spec::op_base_fee_params_for_block,
    transaction::signed::OpSignedTransaction,
    HaltReason, Hardfork, OpChainSpec,
};

/// Block builder for OP.
pub struct OpBlockBuilder<'builder, BlockchainErrorT: Debug> {
    eth: EthBlockBuilder<
        'builder,
        OpBlockReceipt,
        <OpChainSpec as BlockChainSpec>::Block,
        BlockchainErrorT,
        OpChainSpec,
        OpExecutionReceiptBuilder,
        OpChainSpec,
        LocalBlock,
    >,
}

impl<'builder, BlockchainErrorT: std::error::Error>
    BlockBuilder<'builder, OpChainSpec, OpBlockReceipt, <OpChainSpec as BlockChainSpec>::Block>
    for OpBlockBuilder<'builder, BlockchainErrorT>
{
    type BlockchainError = BlockchainErrorT;

    type LocalBlock = LocalBlock;

    fn new_block_builder(
        blockchain: &'builder dyn edr_blockchain_api::Blockchain<
            OpBlockReceipt,
            <OpChainSpec as BlockChainSpec>::Block,
            Self::BlockchainError,
            Hardfork,
            Self::LocalBlock,
            OpSignedTransaction,
        >,
        state: Box<dyn DynState>,
        evm_config: &EvmConfig,
        mut inputs: BlockInputs,
        mut overrides: HeaderOverrides<Hardfork>,
        custom_precompiles: &'builder HashMap<Address, PrecompileFn>,
    ) -> Result<
        Self,
        BlockBuilderCreationError<
            DatabaseComponentError<Self::BlockchainError, StateError>,
            Hardfork,
        >,
    > {
        let hardfork = blockchain.hardfork();

        let parent_block = blockchain.last_block().map_err(|error| {
            BlockBuilderCreationError::Database(DatabaseComponentError::Blockchain(error))
        })?;

        let parent_header = parent_block.block_header();

        // TODO: https://github.com/NomicFoundation/edr/issues/990
        // Replace this once we can detect chain-specific block inputs in the provider
        // and avoid passing them as input.
        if hardfork >= Hardfork::CANYON {
            // `EthBlockBuilder` expects `inputs.withdrawals.is_some()` despite OP not
            // supporting withdrawals.
            inputs.withdrawals = Some(Vec::new());
        }

        if hardfork >= Hardfork::ISTHMUS {
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
                .map_err(|error| {
                    BlockBuilderCreationError::Database(DatabaseComponentError::State(error))
                })?;

            overrides.withdrawals_root = Some(withdrawals_root);
        }
        if hardfork >= Hardfork::HOLOCENE {
            // For post-Holocene blocks, store the encoded base fee parameters to be used in
            // the next block as `extraData`. See: <https://specs.optimism.io/protocol/holocene/exec-engine.html>
            overrides.extra_data = Some(overrides.extra_data.unwrap_or_else(|| {
                let chain_base_fee_params = overrides
                    .base_fee_params
                    .as_ref()
                    .unwrap_or_else(|| blockchain.base_fee_params())
                    .clone();

                let current_block_number = blockchain.last_block_number() + 1;
                let next_block_number = current_block_number + 1;

                let extra_data_base_fee_params = chain_base_fee_params
                    .at_condition(hardfork, next_block_number)
                    .expect("Chain spec must have base fee params for post-London hardforks");
                encode_dynamic_base_fee_params(extra_data_base_fee_params)
            }));

            overrides.base_fee_params = if let Some(base_fee_params) = overrides.base_fee_params {
                Some(base_fee_params)
            } else {
                let parent_block_number = blockchain.last_block_number();
                let parent_hardfork = blockchain
                    .spec_at_block_number(parent_block_number)
                    .map_err(|error| {
                        BlockBuilderCreationError::Database(DatabaseComponentError::Blockchain(
                            error,
                        ))
                    })?;

                op_base_fee_params_for_block(parent_header, parent_hardfork)
            };
        }

        let l1_block_info = {
            let l2_block_number = overridden_block_number(Some(parent_header), &overrides);
            let mut db = WrapDatabaseRef(DatabaseComponents {
                blockchain,
                state: state.as_ref(),
            });

            op_revm::L1BlockInfo::try_fetch(&mut db, U256::from(l2_block_number), hardfork)
                .map_err(BlockBuilderCreationError::Database)?
        };

        let eth = EthBlockBuilder::new(
            l1_block_info,
            blockchain,
            state,
            evm_config,
            inputs,
            overrides,
            custom_precompiles,
        )?;

        Ok(Self { eth })
    }

    fn header(&self) -> &PartialHeader {
        self.eth.header()
    }

    fn add_transaction(
        &mut self,
        transaction: OpSignedTransaction,
    ) -> Result<
        (),
        BlockTransactionError<
            DatabaseComponentError<Self::BlockchainError, StateError>,
            <OpSignedTransaction as TransactionValidation>::ValidationError,
        >,
    > {
        self.eth.add_transaction(transaction)
    }

    fn add_transaction_with_inspector<InspectorT>(
        &mut self,
        transaction: OpSignedTransaction,
        inspector: &mut InspectorT,
    ) -> Result<
        (),
        BlockTransactionError<
            DatabaseComponentError<Self::BlockchainError, StateError>,
            <OpSignedTransaction as TransactionValidation>::ValidationError,
        >,
    >
    where
        InspectorT: for<'inspector> edr_evm_spec::Inspector<
            edr_evm_spec::ContextForChainSpec<
                OpChainSpec,
                <OpChainSpec as edr_chain_spec::BlockEnvChainSpec>::BlockEnv<
                    'inspector,
                    PartialHeader,
                >,
                WrapDatabaseRef<
                    DatabaseComponents<
                        &'inspector dyn edr_blockchain_api::Blockchain<
                            OpBlockReceipt,
                            <OpChainSpec as BlockChainSpec>::Block,
                            Self::BlockchainError,
                            Hardfork,
                            Self::LocalBlock,
                            OpSignedTransaction,
                        >,
                        &'inspector dyn DynState,
                    >,
                >,
            >,
        >,
    {
        self.eth
            .add_transaction_with_inspector(transaction, inspector)
    }

    fn finalize_block(
        self,
        rewards: Vec<(Address, u128)>,
    ) -> Result<BuiltBlockAndState<HaltReason, Self::LocalBlock>, StateError> {
        self.eth.finalize(rewards)
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
