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
use edr_napi_core::{
    napi,
    spec::{marshal_response_data, Response, SyncNapiSpec},
};
use edr_primitives::HashMap;
use edr_provider::{time::TimeSinceEpoch, ProviderSpec, TransactionFailureReason};
use edr_receipt::ExecutionReceiptChainSpec;
use edr_rpc_eth::jsonrpc;
use edr_solidity::contract_decoder::ContractDecoder;
use edr_state_api::{StateDebug as _, StateDiff};
use edr_state_persistent_trie::PersistentStateTrie;
use op_revm::{precompiles::OpPrecompiles, L1BlockInfo, OpEvm};
use revm_context::{result::EVMError, CfgEnv, Journal, JournalTr as _};
use serde::{de::DeserializeOwned, Serialize};

use crate::{
    block::{decode_base_params, decode_min_base_fee, LocalBlock, OpBlockBuilder},
    eip1559::encode_dynamic_base_fee_params,
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

    type BlockBuilder<'builder, BlockchainErrorT: 'builder + std::error::Error> =
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
        block_config: BlockConfig<'_, Self::Hardfork>,
        mut options: GenesisBlockOptions<Self::Hardfork>,
    ) -> Result<Self::LocalBlock, Self::GenesisBlockCreationError> {
        let genesis_state = PersistentStateTrie::from(genesis_diff);

        if block_config.hardfork >= Hardfork::HOLOCENE {
            let config_base_fee_params = options.base_fee_params.as_ref();
            // If no option is provided, fill the `extra_data` field with the dynamic
            // EIP-1559 parameters.
            options.extra_data = options.extra_data.or_else(|| {
                let base_fee_params = config_base_fee_params
                    .unwrap_or(block_config.base_fee_params)
                    .at_condition(block_config.hardfork, 0)
                    .expect("Chain spec must have base fee params for post-London hardforks");

                // TODO: once EDR fully supports Jovian, should allow user to configure
                // min_base_fee?
                let min_base_fee = if block_config.hardfork >= Hardfork::JOVIAN {
                    Some(0)
                } else {
                    None
                };
                Some(encode_dynamic_base_fee_params(
                    base_fee_params,
                    min_base_fee,
                ))
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

/// Calculate next `base_fee` for OP stack block
///
/// If Jovian is activated, clamps the standard EVM calculation result to the
/// minimum encoded in `extra_data` field See <https://specs.optimism.io/protocol/jovian/exec-engine.html#minimum-base-fee-in-block-header>
pub(crate) fn op_next_base_fee(
    parent_header: &BlockHeader,
    hardfork: Hardfork,
    base_fee_params: &BaseFeeParams<Hardfork>,
) -> u128 {
    let base_fee_per_gas =
        calculate_next_base_fee_per_gas(parent_header, base_fee_params, hardfork);
    if hardfork >= Hardfork::JOVIAN {
        let min_base_fee = decode_min_base_fee(&parent_header.extra_data)
            .expect("Jovian should have min base fee defined in extra data");
        if base_fee_per_gas < min_base_fee {
            min_base_fee
        } else {
            base_fee_per_gas
        }
    } else {
        base_fee_per_gas
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
        response: Result<
            edr_provider::ResponseWithTraces<HaltReason>,
            edr_provider::ProviderErrorForChainSpec<Self>,
        >,
        _contract_decoder: Arc<ContractDecoder>,
    ) -> napi::Result<edr_napi_core::spec::Response<EvmHaltReason>> {
        let response = jsonrpc::ResponseData::from(response.map(|response| response.result));

        marshal_response_data(response).map(|data| Response {
            data,
            // TODO: Add support for Solidity stack traces in OP
            solidity_trace: None,
            traces: Vec::new(),
        })
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

        let block =
            <OpChainSpec as BlockEnvChainSpec>::BlockEnv::new_block_env(&header, Hardfork::ECOTONE);
        assert_eq!(block.blob_excess_gas_and_price(), None);
    }

    #[test]
    fn op_block_constructor_should_not_default_excess_blob_gas_before_cancun() {
        let header = build_block_header(None); // No blob gas information

        let block =
            <OpChainSpec as BlockEnvChainSpec>::BlockEnv::new_block_env(&header, Hardfork::CANYON);
        assert_eq!(block.blob_excess_gas_and_price(), None);
    }

    #[test]
    fn op_block_constructor_should_not_default_excess_blob_gas_after_cancun() {
        let header = build_block_header(None); // No blob gas information

        let block =
            <OpChainSpec as BlockEnvChainSpec>::BlockEnv::new_block_env(&header, Hardfork::ISTHMUS);
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

        let block =
            <OpChainSpec as BlockEnvChainSpec>::BlockEnv::new_block_env(&header, Hardfork::ECOTONE);

        let blob_excess_gas = block
            .blob_excess_gas_and_price()
            .expect("Blob excess gas should be set");
        assert_eq!(blob_excess_gas.excess_blob_gas, excess_gas);
    }
}
