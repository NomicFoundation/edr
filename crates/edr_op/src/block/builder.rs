use core::fmt::Debug;

use edr_block_builder_api::{
    BlockBuilder, BlockBuilderCreationError, BlockFinalizeError, BlockInputs,
    BlockTransactionError, BuiltBlockAndState, DatabaseComponents, PrecompileFn, WrapDatabaseRef,
};
use edr_block_header::{
    overridden_block_number, HeaderOverrides, PartialHeader,
};
use edr_chain_l1::block::EthBlockBuilder;
use edr_chain_spec::TransactionValidation;
use edr_chain_spec_block::BlockChainSpec;
use edr_chain_spec_evm::{config::EvmConfig, DatabaseComponentError};
use edr_eip1559::ConstantBaseFeeParams;
use edr_primitives::{Address, Bytes, HashMap, B256, KECCAK_NULL_RLP, U256};
use edr_state_api::{DynState, StateError};

use crate::{
    block::LocalBlock,
    eip1559::{
        encode_dynamic_base_fee_params, HOLOCENE_BASE_FEE_PARAM_VERSION,
        JOVIAN_BASE_FEE_PARAM_VERSION,
    },
    predeploys::L2_TO_L1_MESSAGE_PASSER_ADDRESS,
    receipt::{block::OpBlockReceipt, execution::OpExecutionReceiptBuilder},
    spec::{op_base_fee_params_for_block, op_next_base_fee},
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

        overrides.withdrawals_root = overrides.withdrawals_root.map_or_else(
            || {
                define_op_withdrawals_root(hardfork, &state).map_err(|error| {
                    BlockBuilderCreationError::Database(DatabaseComponentError::State(error))
                })
            },
            |value| Ok(Some(value)),
        )?;

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
                // TODO: instead of decoding min_base_fee from parent extra data we should get
                // the info from OP chain config analogously to base_fee_params
                encode_dynamic_base_fee_params(
                    extra_data_base_fee_params,
                    decode_min_base_fee(&parent_header.extra_data),
                )
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

        if hardfork >= Hardfork::JOVIAN {
            // since Jovian hardfork base_fee calculation in OP stacks differs from standard EVM calculation
            overrides.base_fee = overrides.base_fee.or_else(|| 
                overrides.base_fee_params.as_ref().map(|base_fee_params| {
                    op_next_base_fee(parent_header, hardfork, &base_fee_params)
                })
            );
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
        InspectorT: for<'inspector> edr_chain_spec_evm::Inspector<
            edr_chain_spec_evm::ContextForChainSpec<
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
    ) -> Result<BuiltBlockAndState<HaltReason, Self::LocalBlock>, BlockFinalizeError<StateError>>
    {
        self.eth.finalize(rewards)
    }
}

/// Prior to isthmus activation: the L2 block header's withdrawalsRoot field
/// must be:
///    - nil if Canyon has not been activated.
///    - `keccak256(rlp(empty_string_code))` if Canyon has been activated.
///
/// After Isthmus activation, the withdrawalsRoot field should be the
/// `L2ToL1MessagePasser` account storage root
fn define_op_withdrawals_root(
    hardfork: Hardfork,
    state: &dyn DynState,
) -> Result<Option<B256>, StateError> {
    if hardfork < Hardfork::CANYON {
        Ok(None)
    } else if hardfork < Hardfork::ISTHMUS {
        Ok(Some(KECCAK_NULL_RLP))
    } else {
        state.account_storage_root(&L2_TO_L1_MESSAGE_PASSER_ADDRESS)
    }
}
/// Decodes the base fee params from Bytes considering op-stack extra-param spec
pub fn decode_base_params(extra_data: &Bytes) -> ConstantBaseFeeParams {
    let version = *extra_data
        .first()
        .expect("Extra data should have at least 1 byte for version");
    match version {
        HOLOCENE_BASE_FEE_PARAM_VERSION | JOVIAN_BASE_FEE_PARAM_VERSION => {
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
            "Unsupported base fee params version: {version}. Expected up to {JOVIAN_BASE_FEE_PARAM_VERSION}."
        )
    }
}

/// extract min base fee from block header extra data
pub fn decode_min_base_fee(extra_data: &Bytes) -> Option<u128> {
    let version = *extra_data
        .first()
        .expect("Extra data should have at least 1 byte for version");
    match version {
        HOLOCENE_BASE_FEE_PARAM_VERSION => None,
        JOVIAN_BASE_FEE_PARAM_VERSION => {
            let min_base_fee_bytes: [u8; 8] = extra_data
                .get(9..=16)
                .expect("Extra data should have at least 17 bytes for dynamic base fee params")
                .try_into()
                .expect("The slice should be exactly 8 bytes");

            let min_base_fee = u64::from_be_bytes(min_base_fee_bytes).into();
            Some(min_base_fee)
        },
        _ => panic!(
            "Unsupported base fee params version: {version}. Expected up to {JOVIAN_BASE_FEE_PARAM_VERSION}."
        )
}
}
#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use edr_block_api::{GenesisBlockFactory, GenesisBlockOptions};
    use edr_block_header::BlockConfig;
    use edr_blockchain_api::StateAtBlock as _;
    use edr_chain_spec_provider::ProviderChainSpec;
    use edr_provider::spec::LocalBlockchainForChainSpec;
    use edr_state_api::StateDiff;

    use super::*;
    use crate::{block::builder::define_op_withdrawals_root, hardfork::op, OpChainSpec};

    fn create_local_blockchain(
        hardfork: Hardfork,
    ) -> anyhow::Result<LocalBlockchainForChainSpec<OpChainSpec>> {
        let block_config = BlockConfig {
            hardfork,
            base_fee_params: &op::MAINNET_BASE_FEE_PARAMS,
            min_ethash_difficulty: OpChainSpec::MIN_ETHASH_DIFFICULTY,
        };
        let genesis_block = OpChainSpec::genesis_block(
            StateDiff::default(),
            block_config.clone(),
            GenesisBlockOptions {
                mix_hash: Some(B256::ZERO),
                ..GenesisBlockOptions::default()
            },
        )?;

        Ok(LocalBlockchainForChainSpec::<OpChainSpec>::new(
            genesis_block,
            StateDiff::default(),
            1234,
            block_config,
        )?)
    }

    #[test]
    fn should_return_none_if_before_canyon() -> anyhow::Result<()> {
        let hardfork = Hardfork::BEDROCK;
        let blockchain = create_local_blockchain(hardfork)?;
        let state = blockchain.state_at_block_number(0, &BTreeMap::new())?;
        let response = define_op_withdrawals_root(hardfork, &state);
        assert_eq!(response.unwrap(), None);
        Ok(())
    }
    #[test]
    fn should_return_keccak_zero_if_canyon() -> anyhow::Result<()> {
        let hardfork = Hardfork::CANYON;
        let blockchain = create_local_blockchain(hardfork)?;
        let state = blockchain.state_at_block_number(0, &BTreeMap::new())?;
        let response = define_op_withdrawals_root(hardfork, &state);
        assert_eq!(response.unwrap(), Some(KECCAK_NULL_RLP));
        Ok(())
    }

    #[test]
    fn should_return_l2l1passer_storage_root_if_isthmus() -> anyhow::Result<()> {
        let hardfork = Hardfork::ISTHMUS;
        let blockchain = create_local_blockchain(hardfork)?;
        let state = blockchain.state_at_block_number(0, &BTreeMap::new())?;
        let response = define_op_withdrawals_root(hardfork, &state);
        assert_eq!(
            response.unwrap(),
            state
                .account_storage_root(&L2_TO_L1_MESSAGE_PASSER_ADDRESS)
                .unwrap()
        );
        Ok(())
    }
}
