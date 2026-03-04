use core::fmt::Debug;
use std::sync::Arc;

use alloy_rlp::RlpEncodable;
use edr_block_api::{sync::SyncBlock, GenesisBlockFactory, GenesisBlockOptions};
use edr_block_header::{
    calculate_next_base_fee_per_gas, BlockConfig, BlockHeader, HeaderAndEvmSpec,
};
use edr_block_local::LocalBlockCreationError;
use edr_block_remote::FetchRemoteReceiptError;
use edr_chain_config::ChainConfig;
use edr_chain_l1::rpc::{call::L1CallRequest, TransactionRequest};
use edr_chain_spec::{
    BlockEnvChainSpec, ChainSpec, ContextChainSpec, EvmHaltReason, EvmTransactionValidationError,
    HardforkChainSpec, TransactionValidation,
};
use edr_chain_spec_block::BlockChainSpec;
use edr_chain_spec_evm::{
    handler::EthInstructions, Context, ContextForChainSpec, Database, Evm, EvmChainSpec,
    ExecuteEvm as _, ExecutionResultAndState, InspectEvm as _, InterpreterResult, LocalContext,
    PrecompileProvider, TransactionError,
};
use edr_chain_spec_provider::ProviderChainSpec;
use edr_chain_spec_receipt::ReceiptChainSpec;
use edr_chain_spec_rpc::{RpcBlockChainSpec, RpcChainSpec};
use edr_eip1559::BaseFeeParams;
use edr_eip7892::ScheduledBlobParams;
use edr_napi_core::{
    napi,
    spec::{cast_provider_result_to_response, SyncNapiSpec},
};
use edr_primitives::HashMap;
use edr_provider::{
    time::TimeSinceEpoch, ProviderErrorForChainSpec, ProviderSpec, ResponseWithCallTraces,
    TransactionFailureReason,
};
use edr_receipt::ExecutionReceiptChainSpec;
use edr_state_api::{StateDebug as _, StateDiff};
use edr_state_persistent_trie::PersistentStateTrie;
use op_revm::{precompiles::OpPrecompiles, L1BlockInfo, OpEvm};
use revm_context::{result::EVMError, CfgEnv, Journal, JournalTr as _};
use serde::{de::DeserializeOwned, Serialize};

use crate::{
    block::{decode_base_params, decode_min_base_fee, LocalBlock, OpBlockBuilder},
    eip1559::{encode_dynamic_base_fee_params_holocene, encode_dynamic_base_fee_params_jovian},
    eip2718::TypedEnvelope,
    hardfork::{op_chain_configs, op_default_base_fee_params},
    predeploys::L2_TO_L1_MESSAGE_PASSER_ADDRESS,
    receipt::{
        block::OpBlockReceipt,
        execution::{OpExecutionReceipt, OpExecutionReceiptBuilder},
    },
    rpc,
    transaction::{
        pooled::OpPooledTransaction, request::OpTransactionRequest, signed::OpSignedTransaction,
    },
    HaltReason, Hardfork, InvalidTransaction,
};

fn cast_evm_error<DatabaseErrorT: Debug + std::error::Error>(
    error: EVMError<DatabaseErrorT, InvalidTransaction>,
) -> TransactionError<DatabaseErrorT, InvalidTransaction> {
    match error {
        EVMError::Custom(error) => TransactionError::Custom(error),
        EVMError::Database(error) => TransactionError::Database(error),
        EVMError::Header(error) => TransactionError::InvalidHeader(error),
        EVMError::Transaction(error) => {
            if let InvalidTransaction::Base(EvmTransactionValidationError::LackOfFundForMaxFee {
                fee,
                balance,
            }) = error
            {
                TransactionError::LackOfFundForMaxFee { fee, balance }
            } else {
                TransactionError::InvalidTransaction(error)
            }
        }
    }
}

/// Chain specification for the Ethereum JSON-RPC API.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, RlpEncodable)]
pub struct OpChainSpec;

impl BlockChainSpec for OpChainSpec {
    type Block =
        dyn SyncBlock<Arc<Self::Receipt>, Self::SignedTransaction, Error = Self::FetchReceiptError>;

    type BlockBuilder<'builder, BlockchainErrorT: 'builder + std::error::Error + Send + Sync + 'static> =
        OpBlockBuilder<'builder, BlockchainErrorT>;

    type FetchReceiptError =
        FetchRemoteReceiptError<<Self::Receipt as TryFrom<Self::RpcReceipt>>::Error>;
}

impl BlockEnvChainSpec for OpChainSpec {
    type BlockEnv<'header, BlockHeaderT>
        = HeaderAndEvmSpec<'header, BlockHeaderT, Self::Hardfork>
    where
        BlockHeaderT: 'header + edr_chain_spec::BlockEnvForHardfork<Self::Hardfork>;
}

impl ChainSpec for OpChainSpec {
    type HaltReason = HaltReason;
    type SignedTransaction = OpSignedTransaction;
}

impl ContextChainSpec for OpChainSpec {
    type Context = L1BlockInfo;
}

impl EvmChainSpec for OpChainSpec {
    type PrecompileProvider<BlockT: revm_context::Block, DatabaseT: Database> = OpPrecompiles;

    fn dry_run<
        BlockT: revm_context::Block,
        DatabaseT: Database,
        PrecompileProviderT: PrecompileProvider<
            ContextForChainSpec<Self, BlockT, DatabaseT>,
            Output = InterpreterResult,
        >,
    >(
        block: BlockT,
        cfg: CfgEnv<Self::Hardfork>,
        transaction: Self::SignedTransaction,
        mut database: DatabaseT,
        precompile_provider: PrecompileProviderT,
    ) -> Result<
        ExecutionResultAndState<Self::HaltReason>,
        TransactionError<
            DatabaseT::Error,
            <Self::SignedTransaction as TransactionValidation>::ValidationError,
        >,
    > {
        let chain = L1BlockInfo::try_fetch(&mut database, block.number(), cfg.spec)
            .map_err(TransactionError::Database)?;

        let context = Context {
            block,
            tx: transaction,
            journaled_state: Journal::new(database),
            cfg,
            chain,
            local: LocalContext::default(),
            error: Ok(()),
        };

        let mut evm = OpEvm(Evm::new(
            context,
            EthInstructions::new_mainnet(),
            precompile_provider,
        ));

        evm.replay().map_err(cast_evm_error)
    }

    fn dry_run_with_inspector<
        BlockT: revm_context::Block,
        DatabaseT: revm_context::Database,
        InspectorT: edr_chain_spec_evm::Inspector<ContextForChainSpec<Self, BlockT, DatabaseT>>,
        PrecompileProviderT: PrecompileProvider<
            ContextForChainSpec<Self, BlockT, DatabaseT>,
            Output = InterpreterResult,
        >,
    >(
        block: BlockT,
        cfg: CfgEnv<Self::Hardfork>,
        transaction: Self::SignedTransaction,
        mut database: DatabaseT,
        precompile_provider: PrecompileProviderT,
        inspector: InspectorT,
    ) -> Result<
        ExecutionResultAndState<Self::HaltReason>,
        TransactionError<
            DatabaseT::Error,
            <Self::SignedTransaction as TransactionValidation>::ValidationError,
        >,
    > {
        let chain = L1BlockInfo::try_fetch(&mut database, block.number(), cfg.spec)
            .map_err(TransactionError::Database)?;

        let context = Context {
            block,
            // We need to pass a transaction here to properly initialize the context.
            // This default transaction is immediately overridden by the actual transaction passed
            // to `InspectEvm::inspect_tx`, so its values do not affect the inspection
            // process.
            tx: Self::SignedTransaction::default(),
            journaled_state: Journal::new(database),
            cfg,
            chain,
            local: LocalContext::default(),
            error: Ok(()),
        };

        let mut evm = OpEvm(Evm::new_with_inspector(
            context,
            inspector,
            EthInstructions::new_mainnet(),
            precompile_provider,
        ));

        evm.inspect_tx(transaction).map_err(cast_evm_error)
    }
}

impl ExecutionReceiptChainSpec for OpChainSpec {
    type ExecutionReceipt<Log> = TypedEnvelope<OpExecutionReceipt<Log>>;
}

impl GenesisBlockFactory for OpChainSpec {
    type GenesisBlockCreationError = LocalBlockCreationError;

    type LocalBlock = LocalBlock;

    fn genesis_block(
        genesis_diff: StateDiff,
        block_config: &BlockConfig<Self::Hardfork>,
        mut options: GenesisBlockOptions<Self::Hardfork>,
    ) -> Result<Self::LocalBlock, Self::GenesisBlockCreationError> {
        let genesis_state = PersistentStateTrie::from(genesis_diff);

        if block_config.hardfork >= Hardfork::HOLOCENE {
            let config_base_fee_params = options.base_fee_params.as_ref();
            // If no option is provided, fill the `extra_data` field with the dynamic
            // EIP-1559 parameters.
            options.extra_data = options.extra_data.or_else(|| {
                let base_fee_params = config_base_fee_params
                    .unwrap_or(&block_config.base_fee_params)
                    .at_condition(block_config.hardfork, 0)
                    .expect("Chain spec must have base fee params for post-London hardforks");

                let encoded_extra_data = if block_config.hardfork >= Hardfork::JOVIAN {
                    // TODO: once EDR fully supports Jovian, should allow user to configure
                    // min_base_fee?
                    encode_dynamic_base_fee_params_jovian(base_fee_params, 0)
                } else {
                    encode_dynamic_base_fee_params_holocene(base_fee_params)
                };
                Some(encoded_extra_data)
            });
        }

        if block_config.hardfork >= Hardfork::ISTHMUS {
            let withdrawals_root = options.withdrawals_root.map_or_else(
                || genesis_state.account_storage_root(&L2_TO_L1_MESSAGE_PASSER_ADDRESS),
                |value| Ok(Some(value)),
            )?;
            options.withdrawals_root = withdrawals_root;
        };
        LocalBlock::with_genesis_state(genesis_state, block_config, options)
    }
}

impl HardforkChainSpec for OpChainSpec {
    type Hardfork = Hardfork;
}

/// Returns the base fee parameters to be used for the current block.
pub(crate) fn op_base_fee_params_for_block(
    parent_header: &BlockHeader,
    parent_hardfork: Hardfork,
) -> Option<BaseFeeParams<Hardfork>> {
    // For post-Holocene blocks, use the parent header extra_data to determine the
    // base fee parameters
    if parent_hardfork >= Hardfork::HOLOCENE {
        Some(BaseFeeParams::Constant(decode_base_params(
            &parent_header.extra_data,
        )))
    } else {
        None
    }
}

/// Calculates the next block's `base_fee` for an OP stack chain.
///
/// Pre-Jovian: applies the standard EIP-1559 update over the parent's
/// `gas_used`.
///
/// From Jovian onward, two changes apply:
/// - `gasMetered := max(gasUsed, blobGasUsed)` is used in place of `gasUsed`,
///   where `blobGasUsed` carries the parent block's DA footprint. See
///   <https://specs.optimism.io/protocol/jovian/exec-engine.html#da-footprint-block-limit>.
/// - The result is clamped to the minimum base fee encoded in the parent's
///   `extra_data`. See
///   <https://specs.optimism.io/protocol/jovian/exec-engine.html#minimum-base-fee-in-block-header>.
pub(crate) fn op_next_base_fee(
    parent_header: &BlockHeader,
    hardfork: Hardfork,
    base_fee_params: &BaseFeeParams<Hardfork>,
) -> u128 {
    if hardfork >= Hardfork::JOVIAN {
        let parent_blob_gas_used = parent_header
            .blob_gas
            .as_ref()
            .map_or(0, |blob_gas| u128::from(blob_gas.gas_used));

        let gas_metered = core::cmp::max(u128::from(parent_header.gas_used), parent_blob_gas_used);
        let base_fee_per_gas =
            calculate_next_base_fee_per_gas(parent_header, gas_metered, base_fee_params, hardfork);

        let min_base_fee = decode_min_base_fee(&parent_header.extra_data)
            .expect("Jovian should have min base fee defined in extra data");

        core::cmp::max(base_fee_per_gas, min_base_fee)
    } else {
        calculate_next_base_fee_per_gas(
            parent_header,
            u128::from(parent_header.gas_used),
            base_fee_params,
            hardfork,
        )
    }
}

impl ProviderChainSpec for OpChainSpec {
    const MIN_ETHASH_DIFFICULTY: u64 = 0;

    fn chain_configs() -> &'static HashMap<u64, ChainConfig<Self::Hardfork>> {
        op_chain_configs()
    }

    fn default_base_fee_params() -> &'static BaseFeeParams<Self::Hardfork> {
        op_default_base_fee_params()
    }

    fn next_base_fee_per_gas(
        header: &BlockHeader,
        hardfork: Self::Hardfork,
        default_base_fee_params: &BaseFeeParams<Self::Hardfork>,
    ) -> u128 {
        let block_base_fee_params = op_base_fee_params_for_block(header, hardfork);

        op_next_base_fee(
            header,
            hardfork,
            block_base_fee_params
                .as_ref()
                .unwrap_or(default_base_fee_params),
        )
    }

    fn default_schedulded_blob_params() -> Option<ScheduledBlobParams> {
        None
    }
}

impl ReceiptChainSpec for OpChainSpec {
    type ExecutionReceiptBuilder = OpExecutionReceiptBuilder;

    type Receipt = OpBlockReceipt;
}

impl RpcBlockChainSpec for OpChainSpec {
    type RpcBlock<Data>
        = edr_chain_l1::rpc::Block<Data>
    where
        Data: DeserializeOwned + Serialize;
}

impl RpcChainSpec for OpChainSpec {
    type RpcCallRequest = L1CallRequest;
    type RpcReceipt = rpc::OpRpcBlockReceipt;
    type RpcTransaction = rpc::Transaction;
    type RpcTransactionRequest = TransactionRequest;
}

impl<TimerT: Clone + TimeSinceEpoch> ProviderSpec<TimerT> for OpChainSpec {
    type PooledTransaction = OpPooledTransaction;
    type TransactionRequest = OpTransactionRequest;

    fn cast_halt_reason(reason: HaltReason) -> TransactionFailureReason<HaltReason> {
        match reason {
            HaltReason::Base(reason) => match reason {
                EvmHaltReason::CreateContractSizeLimit => {
                    TransactionFailureReason::CreateContractSizeLimit
                }
                EvmHaltReason::OpcodeNotFound | EvmHaltReason::InvalidFEOpcode => {
                    TransactionFailureReason::OpcodeNotFound
                }
                EvmHaltReason::OutOfGas(error) => TransactionFailureReason::OutOfGas(error),
                remainder => TransactionFailureReason::Inner(HaltReason::Base(remainder)),
            },
            remainder @ HaltReason::FailedDeposit => TransactionFailureReason::Inner(remainder),
        }
    }
}

impl<TimerT: Clone + TimeSinceEpoch> SyncNapiSpec<TimerT> for OpChainSpec {
    const CHAIN_TYPE: &'static str = crate::CHAIN_TYPE;

    fn cast_response(
        response: Result<ResponseWithCallTraces, ProviderErrorForChainSpec<Self>>,
    ) -> napi::Result<edr_napi_core::spec::Response> {
        cast_provider_result_to_response(response)
    }
}

#[cfg(test)]
mod tests {

    use edr_block_header::BlobGas;
    use edr_chain_spec::{BlockEnvConstructor as _, BlockEnvTrait as _};
    use edr_primitives::{Address, Bloom, Bytes, B256, B64, U256};

    use super::*;
    use crate::spec::OpChainSpec;

    fn build_block_header(blob_gas: Option<BlobGas>) -> BlockHeader {
        BlockHeader {
            parent_hash: B256::default(),
            ommers_hash: B256::default(),
            beneficiary: Address::default(),
            state_root: B256::default(),
            transactions_root: B256::default(),
            receipts_root: B256::default(),
            logs_bloom: Bloom::default(),
            difficulty: U256::default(),
            number: 124,
            gas_limit: u64::default(),
            gas_used: 1337,
            timestamp: 0,
            extra_data: Bytes::default(),
            mix_hash: B256::default(),
            nonce: B64::from(99u64),
            base_fee_per_gas: None,
            withdrawals_root: None,
            blob_gas,
            parent_beacon_block_root: None,
            requests_hash: Some(B256::random()),
        }
    }

    #[test]
    fn op_block_constructor_should_not_default_excess_blob_gas_for_cancun() {
        let header = build_block_header(None); // No blob gas information

        let block = <OpChainSpec as BlockEnvChainSpec>::BlockEnv::new_block_env(
            &header,
            Hardfork::ECOTONE,
            None,
        );
        assert_eq!(block.blob_excess_gas_and_price(), None);
    }

    #[test]
    fn op_block_constructor_should_not_default_excess_blob_gas_before_cancun() {
        let header = build_block_header(None); // No blob gas information

        let block = <OpChainSpec as BlockEnvChainSpec>::BlockEnv::new_block_env(
            &header,
            Hardfork::CANYON,
            None,
        );
        assert_eq!(block.blob_excess_gas_and_price(), None);
    }

    #[test]
    fn op_block_constructor_should_not_default_excess_blob_gas_after_cancun() {
        let header = build_block_header(None); // No blob gas information

        let block = <OpChainSpec as BlockEnvChainSpec>::BlockEnv::new_block_env(
            &header,
            Hardfork::ISTHMUS,
            None,
        );
        assert_eq!(block.blob_excess_gas_and_price(), None);
    }

    #[test]
    fn op_block_constructor_should_use_existing_excess_blob_gas() {
        let excess_gas = 0x80000u64;
        let blob_gas = BlobGas {
            excess_gas,
            gas_used: 0x80000u64,
        };
        let header = build_block_header(Some(blob_gas)); // blob gas present

        let block = <OpChainSpec as BlockEnvChainSpec>::BlockEnv::new_block_env(
            &header,
            Hardfork::ECOTONE,
            None,
        );

        let blob_excess_gas = block
            .blob_excess_gas_and_price()
            .expect("Blob excess gas should be set");
        assert_eq!(blob_excess_gas.excess_blob_gas, excess_gas);
    }

    mod op_next_base_fee {
        use edr_eip1559::ConstantBaseFeeParams;

        use super::*;

        // With these parameters, `gas_target = gas_limit / elasticity = 5M`.
        const GAS_LIMIT: u64 = 30_000_000;
        const GAS_TARGET: u64 = 5_000_000;
        const PARENT_BASE_FEE: u128 = 1_000_000_000;
        const BASE_FEE_PARAMS: ConstantBaseFeeParams = ConstantBaseFeeParams {
            max_change_denominator: 300,
            elasticity_multiplier: 6,
        };

        fn parent_with(
            gas_used: u64,
            blob_gas_used: Option<u64>,
            extra_data: Bytes,
        ) -> BlockHeader {
            BlockHeader {
                gas_limit: GAS_LIMIT,
                gas_used,
                base_fee_per_gas: Some(PARENT_BASE_FEE),
                blob_gas: blob_gas_used.map(|gas_used| BlobGas {
                    gas_used,
                    excess_gas: 0,
                }),
                extra_data,
                ..BlockHeader::default()
            }
        }

        #[test]
        fn pre_jovian_ignores_blob_gas_used() {
            // gas_used == target → no-change under standard EIP-1559 if blob is ignored.
            let parent = parent_with(GAS_TARGET, Some(10_000_000), Bytes::default());

            let result = op_next_base_fee(
                &parent,
                Hardfork::HOLOCENE,
                &BaseFeeParams::Constant(BASE_FEE_PARAMS),
            );

            assert_eq!(result, PARENT_BASE_FEE);
        }

        #[test]
        fn jovian_uses_blob_gas_when_larger_than_gas_used() {
            // gas_used == target (would be no-change alone); blob_gas_used = 2 * target
            // should drive the base fee up via gas_metered = max(gas_used, blob_gas_used).
            let extra_data = encode_dynamic_base_fee_params_jovian(&BASE_FEE_PARAMS, 1);
            let parent = parent_with(GAS_TARGET, Some(2 * GAS_TARGET), extra_data);

            let result = op_next_base_fee(
                &parent,
                Hardfork::JOVIAN,
                &BaseFeeParams::Constant(BASE_FEE_PARAMS),
            );

            assert!(
                result > PARENT_BASE_FEE,
                "base fee should increase when blob_gas_used exceeds the target"
            );

            // Equivalent parent with the blob usage moved into gas_used should yield
            // the same next base fee.
            let extra_data = encode_dynamic_base_fee_params_jovian(&BASE_FEE_PARAMS, 1);
            let equivalent_parent = parent_with(2 * GAS_TARGET, None, extra_data);
            let equivalent_result = op_next_base_fee(
                &equivalent_parent,
                Hardfork::JOVIAN,
                &BaseFeeParams::Constant(BASE_FEE_PARAMS),
            );
            assert_eq!(result, equivalent_result);
        }

        #[test]
        fn jovian_uses_gas_used_when_larger_than_blob_gas() {
            let extra_data = encode_dynamic_base_fee_params_jovian(&BASE_FEE_PARAMS, 1);
            // gas_used > target, blob_gas_used tiny → gas_metered = gas_used.
            let parent = parent_with(2 * GAS_TARGET, Some(1_000), extra_data);

            let jovian_result = op_next_base_fee(
                &parent,
                Hardfork::JOVIAN,
                &BaseFeeParams::Constant(BASE_FEE_PARAMS),
            );

            // Pre-Jovian result for the same header should match, since gas_used dominates.
            let pre_jovian_parent = parent_with(2 * GAS_TARGET, Some(1_000), Bytes::default());
            let pre_jovian_result = op_next_base_fee(
                &pre_jovian_parent,
                Hardfork::HOLOCENE,
                &BaseFeeParams::Constant(BASE_FEE_PARAMS),
            );
            assert_eq!(jovian_result, pre_jovian_result);
        }

        #[test]
        fn jovian_treats_missing_blob_gas_as_zero() {
            let extra_data = encode_dynamic_base_fee_params_jovian(&BASE_FEE_PARAMS, 1);
            let with_blob_zero = parent_with(2 * GAS_TARGET, Some(0), extra_data.clone());
            let without_blob = parent_with(2 * GAS_TARGET, None, extra_data);

            let result_with_blob_zero = op_next_base_fee(
                &with_blob_zero,
                Hardfork::JOVIAN,
                &BaseFeeParams::Constant(BASE_FEE_PARAMS),
            );
            let result_without_blob = op_next_base_fee(
                &without_blob,
                Hardfork::JOVIAN,
                &BaseFeeParams::Constant(BASE_FEE_PARAMS),
            );

            assert_eq!(result_with_blob_zero, result_without_blob);
        }

        #[test]
        fn pre_jovian_does_not_clamp_to_min_base_fee() {
            // Even with a Jovian-encoded extra_data carrying a high min_base_fee, the
            // pre-Jovian path must not apply the clamp — it should ignore extra_data
            // entirely.
            let min_base_fee = 999_000_000u128;
            let extra_data = encode_dynamic_base_fee_params_jovian(&BASE_FEE_PARAMS, min_base_fee);
            // gas_used well under target → base fee decreases below min_base_fee.
            let parent = parent_with(1_000_000, None, extra_data);

            let result = op_next_base_fee(
                &parent,
                Hardfork::HOLOCENE,
                &BaseFeeParams::Constant(BASE_FEE_PARAMS),
            );

            let expected = calculate_next_base_fee_per_gas(
                &parent,
                u128::from(parent.gas_used),
                &BaseFeeParams::Constant(BASE_FEE_PARAMS),
                Hardfork::HOLOCENE,
            );
            assert_eq!(result, expected);
            assert!(
                result < min_base_fee,
                "pre-Jovian result should be below min_base_fee (i.e. un-clamped)"
            );
        }

        #[test]
        fn jovian_clamps_to_min_base_fee() {
            let min_base_fee = 999_000_000u128;
            let extra_data = encode_dynamic_base_fee_params_jovian(&BASE_FEE_PARAMS, min_base_fee);
            // gas_used well under target → unclamped base fee decreases below min_base_fee.
            let parent = parent_with(GAS_TARGET / 5, None, extra_data);

            let result = op_next_base_fee(
                &parent,
                Hardfork::JOVIAN,
                &BaseFeeParams::Constant(BASE_FEE_PARAMS),
            );

            // Sanity: the un-clamped result would be below min_base_fee.
            let unclamped = calculate_next_base_fee_per_gas(
                &parent,
                u128::from(parent.gas_used),
                &BaseFeeParams::Constant(BASE_FEE_PARAMS),
                Hardfork::JOVIAN,
            );
            assert!(unclamped < min_base_fee);
            assert_eq!(result, min_base_fee);
        }

        #[test]
        fn jovian_does_not_clamp_when_above_min_base_fee() {
            let min_base_fee = 1u128;
            let extra_data = encode_dynamic_base_fee_params_jovian(&BASE_FEE_PARAMS, min_base_fee);
            let parent = parent_with(1_000_000, None, extra_data);

            let result = op_next_base_fee(
                &parent,
                Hardfork::JOVIAN,
                &BaseFeeParams::Constant(BASE_FEE_PARAMS),
            );

            let expected = calculate_next_base_fee_per_gas(
                &parent,
                u128::from(parent.gas_used),
                &BaseFeeParams::Constant(BASE_FEE_PARAMS),
                Hardfork::JOVIAN,
            );
            assert_eq!(result, expected);
            assert!(result > min_base_fee);
        }
    }
}
