use std::sync::Arc;

use alloy_rlp::RlpEncodable;
use edr_block_api::{sync::SyncBlock, BlockReceipts, GenesisBlockFactory, GenesisBlockOptions};
use edr_block_header::{BlockConfig, HeaderAndEvmSpec};
use edr_block_local::{EthLocalBlock, LocalBlockCreationError};
use edr_chain_spec::{
    BlockEnvChainSpec, BlockEnvForHardfork, ChainSpec, ContextChainSpec,
    EvmTransactionValidationError, HardforkChainSpec, TransactionValidation,
};
use edr_chain_spec_block::BlockChainSpec;
use edr_evm_spec::{
    handler::{EthInstructions, EthPrecompiles},
    interpreter::InterpreterResult,
    result::EVMError,
    BlockEnvTrait, CfgEnv, Context, ContextForChainSpec, Database, Evm, EvmChainSpec,
    ExecuteEvm as _, ExecutionResultAndState, InspectEvm as _, Inspector, Journal, LocalContext,
    PrecompileProvider, TransactionError,
};
use edr_primitives::Bytes;
use edr_receipt::{log::FilterLog, ExecutionReceiptChainSpec};
use edr_receipt_spec::ReceiptChainSpec;
use edr_rpc_eth::RpcBlockChainSpec;
use edr_rpc_spec::RpcChainSpec;
use edr_state_api::StateDiff;
use revm_context_interface::JournalTr as _;
use serde::{de::DeserializeOwned, Serialize};

use crate::{
    block::EthBlockBuilder,
    receipt::{builder::L1ExecutionReceiptBuilder, L1BlockReceipt},
    rpc::{
        block::L1RpcBlock,
        call::L1CallRequest,
        receipt::L1RpcTransactionReceipt,
        transaction::{L1RpcTransactionRequest, L1RpcTransactionWithSignature},
    },
    HaltReason, Hardfork, L1SignedTransaction, TypedEnvelope,
};

/// Ethereum L1 extra data for genesis blocks.
pub const EXTRA_DATA: &[u8] = b"\x12\x34";

/// The chain specification for Ethereum Layer 1.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, RlpEncodable)]
pub struct L1ChainSpec;

fn cast_evm_error<DatabaseT: Database>(
    error: EVMError<DatabaseT::Error, EvmTransactionValidationError>,
) -> TransactionError<DatabaseT::Error, EvmTransactionValidationError> {
    match error {
        EVMError::Custom(error) => TransactionError::Custom(error),
        EVMError::Database(error) => TransactionError::Database(error),
        EVMError::Header(error) => TransactionError::InvalidHeader(error),
        EVMError::Transaction(EvmTransactionValidationError::LackOfFundForMaxFee {
            fee,
            balance,
        }) => TransactionError::LackOfFundForMaxFee { fee, balance },
        EVMError::Transaction(error) => TransactionError::InvalidTransaction(error),
    }
}

impl BlockChainSpec for L1ChainSpec {
    type Block = dyn SyncBlock<
        Arc<Self::Receipt>,
        Self::SignedTransaction,
        Error = <Self::LocalBlock as BlockReceipts<Arc<Self::Receipt>>>::Error,
    >;

    type BlockBuilder<
        'builder,
        BlockchainErrorT: 'builder + std::error::Error,
        StateErrorT: 'builder + std::error::Error,
    > = EthBlockBuilder<
        'builder,
        Self::Receipt,
        Self::Block,
        BlockchainErrorT,
        Self,
        Self::ExecutionReceiptBuilder,
        Self,
        Self::LocalBlock,
        StateErrorT,
    >;

    type LocalBlock = EthLocalBlock<Self::Receipt, Self::Hardfork, Self::SignedTransaction>;
}

impl BlockEnvChainSpec for L1ChainSpec {
    type BlockEnv<'header, BlockHeaderT>
        = HeaderAndEvmSpec<'header, BlockHeaderT, Self::Hardfork>
    where
        BlockHeaderT: 'header + BlockEnvForHardfork<Self::Hardfork>;
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

        evm.replay().map_err(cast_evm_error::<DatabaseT>)
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

        evm.inspect_tx(transaction)
            .map_err(cast_evm_error::<DatabaseT>)
    }
}

impl ExecutionReceiptChainSpec for L1ChainSpec {
    type ExecutionReceipt<LogT> = TypedEnvelope<edr_receipt::execution::Eip658<LogT>>;
}

impl HardforkChainSpec for L1ChainSpec {
    type Hardfork = Hardfork;
}

impl ContextChainSpec for L1ChainSpec {
    type Context = ();
}

impl ChainSpec for L1ChainSpec {
    type HaltReason = HaltReason;
    type SignedTransaction = L1SignedTransaction;
}

impl GenesisBlockFactory for L1ChainSpec {
    type CreationError = LocalBlockCreationError;

    type LocalBlock = EthLocalBlock<
        <Self as ReceiptChainSpec>::Receipt,
        Self::Hardfork,
        <Self as ChainSpec>::SignedTransaction,
    >;

    fn genesis_block(
        genesis_diff: StateDiff,
        block_config: BlockConfig<'_, Self::Hardfork>,
        mut options: GenesisBlockOptions<Self::Hardfork>,
    ) -> Result<Self::LocalBlock, Self::CreationError> {
        // If no option is provided, use the default extra data for L1 Ethereum.
        options.extra_data = Some(
            options
                .extra_data
                .unwrap_or(Bytes::copy_from_slice(EXTRA_DATA)),
        );

        EthLocalBlock::with_genesis_state(genesis_diff, block_config, options)
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
