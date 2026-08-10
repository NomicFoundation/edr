use std::sync::Arc;

use alloy_rlp::RlpEncodable;
use edr_block_api::{sync::SyncBlock, GenesisBlockFactory, GenesisBlockOptions};
use edr_block_header::{
    calculate_next_base_fee_per_gas, BlockConfig, BlockHeader, HeaderAndEvmSpec,
};
use edr_block_local::{EthLocalBlock, LocalBlockCreationError};
use edr_block_remote::FetchRemoteReceiptError;
use edr_chain_config::ChainConfig;
use edr_chain_spec::{
    BlockEnvChainSpec, BlockEnvForHardfork, ChainSpec, ContextChainSpec, EvmHardforkChainSpec,
    EvmSpecId, ProtocolHardfork as _, ProtocolHardforkChainSpec, TransactionValidation,
};
use edr_chain_spec_block::BlockChainSpec;
use edr_chain_spec_evm::{
    handler::{EthInstructions, EthPrecompiles},
    interpreter::InterpreterResult,
    to_evm_cfg_env, BlockEnvTrait, CfgEnv, Context, ContextForChainSpec, Database, Evm,
    EvmChainSpec, ExecuteEvm as _, ExecutionResultAndState, InspectEvm as _, Inspector, Journal,
    LocalContext, PrecompileProvider, TransactionError,
};
use edr_chain_spec_provider::ProviderChainSpec;
use edr_chain_spec_receipt::ReceiptChainSpec;
use edr_chain_spec_rpc::{RpcBlockChainSpec, RpcChainSpec};
use edr_eip1559::BaseFeeParams;
use edr_eip7892::ScheduledBlobParams;
use edr_primitives::{Bytes, HashMap};
use edr_receipt::{log::FilterLog, ExecutionReceiptChainSpec};
use edr_state_api::StateDiff;
use revm_context_interface::JournalTr as _;
use serde::{de::DeserializeOwned, Serialize};

use crate::{
    block::EthBlockBuilder,
    chains::l1_chain_configs,
    receipt::{builder::L1ExecutionReceiptBuilder, L1BlockReceipt},
    rpc::{
        block::L1RpcBlock,
        call::L1CallRequest,
        receipt::L1RpcTransactionReceipt,
        transaction::{L1RpcTransactionRequest, L1RpcTransactionWithSignature},
    },
    HaltReason, Hardfork, L1SignedTransaction, TypedEnvelope, L1_BASE_FEE_PARAMS,
    L1_GENESIS_BLOCK_EXTRA_DATA, L1_MIN_ETHASH_DIFFICULTY,
};

/// The chain specification for Ethereum Layer 1.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, RlpEncodable)]
pub struct L1ChainSpec;

impl BlockChainSpec for L1ChainSpec {
    type Block =
        dyn SyncBlock<Arc<Self::Receipt>, Self::SignedTransaction, Error = Self::FetchReceiptError>;

    type BlockBuilder<'builder, BlockchainErrorT: 'static + std::error::Error + Send + Sync> =
        EthBlockBuilder<
            'builder,
            Self::Receipt,
            Self::Block,
            BlockchainErrorT,
            Self,
            Self::ExecutionReceiptBuilder,
            Self,
            Self::LocalBlock,
        >;

    type FetchReceiptError =
        FetchRemoteReceiptError<<Self::Receipt as TryFrom<Self::RpcReceipt>>::Error>;
}

impl BlockEnvChainSpec for L1ChainSpec {
    type BlockEnv<'header, BlockHeaderT>
        = HeaderAndEvmSpec<'header, BlockHeaderT, Self::ProtocolHardfork>
    where
        BlockHeaderT: 'header + BlockEnvForHardfork<Self::ProtocolHardfork>;
}

impl ChainSpec for L1ChainSpec {
    type HaltReason = HaltReason;
    type SignedTransaction = L1SignedTransaction;
}

impl ContextChainSpec for L1ChainSpec {
    type Context = ();
}

impl EvmChainSpec for L1ChainSpec {
    type PrecompileProvider<BlockEnvT: BlockEnvTrait, DatabaseT: Database> = EthPrecompiles;

    fn new_precompile_provider<BlockEnvT: BlockEnvTrait, DatabaseT: Database>(
        hardfork: Self::ProtocolHardfork,
    ) -> Self::PrecompileProvider<BlockEnvT, DatabaseT> {
        EthPrecompiles::new(hardfork.to_evm_spec_id())
    }

    fn dry_run<
        BlockEnvT: BlockEnvTrait,
        DatabaseT: Database,
        PrecompileProviderT: PrecompileProvider<
            ContextForChainSpec<Self, BlockEnvT, DatabaseT>,
            Output = InterpreterResult,
        >,
    >(
        block: BlockEnvT,
        cfg: CfgEnv<Self::ProtocolHardfork>,
        transaction: Self::SignedTransaction,
        database: DatabaseT,
        precompile_provider: PrecompileProviderT,
    ) -> Result<
        ExecutionResultAndState<Self::HaltReason>,
        TransactionError<
            DatabaseT::Error,
            <Self::SignedTransaction as TransactionValidation>::ValidationError,
        >,
    > {
        let cfg = to_evm_cfg_env::<Self>(cfg);
        let hardfork = cfg.spec;
        let context = Context {
            block,
            tx: transaction,
            journaled_state: Journal::new(database),
            cfg,
            chain: (),
            local: LocalContext::default(),
            error: Ok(()),
        };

        let mut evm = Evm::new(
            context,
            EthInstructions::new_mainnet_with_spec(hardfork),
            precompile_provider,
        );

        evm.replay().map_err(TransactionError::from)
    }

    fn dry_run_with_inspector<
        BlockEnvT: BlockEnvTrait,
        DatabaseT: Database,
        InspectorT: Inspector<ContextForChainSpec<Self, BlockEnvT, DatabaseT>>,
        PrecompileProviderT: PrecompileProvider<
            ContextForChainSpec<Self, BlockEnvT, DatabaseT>,
            Output = InterpreterResult,
        >,
    >(
        block: BlockEnvT,
        cfg: CfgEnv<Self::ProtocolHardfork>,
        transaction: Self::SignedTransaction,
        database: DatabaseT,
        precompile_provider: PrecompileProviderT,
        inspector: InspectorT,
    ) -> Result<
        ExecutionResultAndState<Self::HaltReason>,
        TransactionError<
            DatabaseT::Error,
            <Self::SignedTransaction as TransactionValidation>::ValidationError,
        >,
    > {
        let cfg = to_evm_cfg_env::<Self>(cfg);
        let hardfork = cfg.spec;
        let context = Context {
            block,
            // We need to pass a transaction here to properly initialize the context.
            // This default transaction is immediately overridden by the actual transaction passed
            // to `InspectEvm::inspect_tx`, so its values do not affect the inspection
            // process.
            tx: Self::SignedTransaction::default(),
            cfg,
            journaled_state: Journal::new(database),
            chain: (),
            local: LocalContext::default(),
            error: Ok(()),
        };

        let mut evm = Evm::new_with_inspector(
            context,
            inspector,
            EthInstructions::new_mainnet_with_spec(hardfork),
            precompile_provider,
        );

        evm.inspect_tx(transaction).map_err(TransactionError::from)
    }
}

impl ExecutionReceiptChainSpec for L1ChainSpec {
    type ExecutionReceipt<LogT> = TypedEnvelope<edr_receipt::Execution<LogT>>;
}

impl GenesisBlockFactory for L1ChainSpec {
    type GenesisBlockCreationError = LocalBlockCreationError;

    type LocalBlock = EthLocalBlock<
        <Self as ReceiptChainSpec>::Receipt,
        <Self as BlockChainSpec>::FetchReceiptError,
        Self::ProtocolHardfork,
        <Self as ChainSpec>::SignedTransaction,
    >;

    fn genesis_block(
        genesis_diff: StateDiff,
        block_config: &BlockConfig<Self::ProtocolHardfork>,
        mut options: GenesisBlockOptions<Self::ProtocolHardfork>,
    ) -> Result<Self::LocalBlock, Self::GenesisBlockCreationError> {
        // If no option is provided, use the default extra data for L1 Ethereum.
        options.extra_data = Some(
            options
                .extra_data
                .unwrap_or(Bytes::copy_from_slice(L1_GENESIS_BLOCK_EXTRA_DATA)),
        );

        EthLocalBlock::with_genesis_state(genesis_diff.into(), block_config, options)
    }
}

impl EvmHardforkChainSpec for L1ChainSpec {
    type EvmHardfork = EvmSpecId;
}

impl ProtocolHardforkChainSpec for L1ChainSpec {
    type ProtocolHardfork = Hardfork;
}

impl ProviderChainSpec for L1ChainSpec {
    const MIN_ETHASH_DIFFICULTY: u64 = L1_MIN_ETHASH_DIFFICULTY;

    fn chain_configs() -> &'static HashMap<u64, ChainConfig<Self::ProtocolHardfork>> {
        l1_chain_configs()
    }

    fn default_base_fee_params() -> &'static BaseFeeParams<Self::ProtocolHardfork> {
        &L1_BASE_FEE_PARAMS
    }

    fn next_base_fee_per_gas(
        header: &BlockHeader,
        hardfork: Self::ProtocolHardfork,
        default_base_fee_params: &BaseFeeParams<Self::ProtocolHardfork>,
    ) -> u128 {
        calculate_next_base_fee_per_gas(
            header,
            u128::from(header.gas_used),
            default_base_fee_params,
            hardfork,
        )
    }

    fn default_schedulded_blob_params() -> Option<ScheduledBlobParams> {
        Some(ScheduledBlobParams::mainnet())
    }
}

impl ReceiptChainSpec for L1ChainSpec {
    type ExecutionReceiptBuilder = L1ExecutionReceiptBuilder;

    type Receipt = L1BlockReceipt<<Self as ExecutionReceiptChainSpec>::ExecutionReceipt<FilterLog>>;
}

impl RpcBlockChainSpec for L1ChainSpec {
    type RpcBlock<DataT>
        = L1RpcBlock<DataT>
    where
        DataT: DeserializeOwned + Serialize;
}

impl RpcChainSpec for L1ChainSpec {
    type RpcCallRequest = L1CallRequest;
    type RpcReceipt = L1RpcTransactionReceipt;
    type RpcTransaction = L1RpcTransactionWithSignature;
    type RpcTransactionRequest = L1RpcTransactionRequest;
}
