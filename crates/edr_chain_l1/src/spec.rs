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
    BlockEnvChainSpec, BlockEnvForHardfork, ChainSpec, ContextChainSpec, HardforkChainSpec,
    TransactionValidation,
};
use edr_chain_spec_block::BlockChainSpec;
use edr_chain_spec_evm::{
    handler::{EthInstructions, EthPrecompiles},
    interpreter::InterpreterResult,
    BlockEnvTrait, CfgEnv, Context, ContextForChainSpec, Database, Evm, EvmChainSpec,
    ExecuteEvm as _, ExecutionResultAndState, InspectEvm as _, Inspector, Journal, LocalContext,
    PrecompileProvider, TransactionError,
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

    type BlockBuilder<'builder, BlockchainErrorT: 'builder + std::error::Error> = EthBlockBuilder<
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
        = HeaderAndEvmSpec<'header, BlockHeaderT, Self::Hardfork>
    where
        BlockHeaderT: 'header + BlockEnvForHardfork<Self::Hardfork>;
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

    fn dry_run<
        BlockEnvT: BlockEnvTrait,
        DatabaseT: Database,
        PrecompileProviderT: PrecompileProvider<
            ContextForChainSpec<Self, BlockEnvT, DatabaseT>,
            Output = InterpreterResult,
        >,
    >(
        block: BlockEnvT,
        cfg: CfgEnv<Self::Hardfork>,
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
        let context = Context {
            block,
            tx: transaction,
            journaled_state: Journal::new(database),
            cfg,
            chain: (),
            local: LocalContext::default(),
            error: Ok(()),
        };

        let mut evm = Evm::new(context, EthInstructions::default(), precompile_provider);

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
        cfg: CfgEnv<Self::Hardfork>,
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
            EthInstructions::default(),
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
        Self::Hardfork,
        <Self as ChainSpec>::SignedTransaction,
    >;

    fn genesis_block(
        genesis_diff: StateDiff,
        block_config: &BlockConfig<Self::Hardfork>,
        mut options: GenesisBlockOptions<Self::Hardfork>,
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

impl HardforkChainSpec for L1ChainSpec {
    type Hardfork = Hardfork;
}

impl ProviderChainSpec for L1ChainSpec {
    const MIN_ETHASH_DIFFICULTY: u64 = L1_MIN_ETHASH_DIFFICULTY;

    fn chain_configs() -> &'static HashMap<u64, ChainConfig<Self::Hardfork>> {
        l1_chain_configs()
    }

    fn default_base_fee_params() -> &'static BaseFeeParams<Self::Hardfork> {
        &L1_BASE_FEE_PARAMS
    }

    fn next_base_fee_per_gas(
        header: &BlockHeader,
        hardfork: Self::Hardfork,
        default_base_fee_params: &BaseFeeParams<Self::Hardfork>,
    ) -> u128 {
        calculate_next_base_fee_per_gas(header, default_base_fee_params, hardfork)
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
