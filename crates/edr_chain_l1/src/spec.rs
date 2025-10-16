use alloy_rlp::RlpEncodable;
use edr_block_api::{GenesisBlockFactory, GenesisBlockOptions};
use edr_block_header::BlockConfig;
use edr_block_local::{EthLocalBlock, LocalBlockCreationError};
use edr_chain_spec::{
    ChainContextSpec, ChainHardfork, ChainSpec, EvmTransactionValidationError,
    TransactionValidation,
};
use edr_evm_spec::{
    handler::{EthInstructions, EthPrecompiles},
    interpreter::InterpreterResult,
    result::EVMError,
    ChainEvmSpec, ContextForChainSpec, Database, Evm, ExecuteEvm as _, InspectEvm as _, Inspector,
    PrecompileProvider, TransactionError,
};
use edr_primitives::Bytes;
use edr_receipt::{log::FilterLog, ExecutionReceiptChainSpec};
use edr_receipt_spec::ChainReceiptSpec;
use edr_rpc_eth::ChainRpcBlock;
use edr_rpc_spec::RpcSpec;
use edr_state_api::StateDiff;
use revm_context::{CfgEnv, Journal, JournalTr as _, LocalContext};
use serde::{de::DeserializeOwned, Serialize};

use crate::{
    receipt::L1BlockReceipt,
    rpc::{
        block::L1RpcBlock,
        call::L1CallRequest,
        receipt::L1RpcTransactionReceipt,
        transaction::{L1RpcTransactionRequest, L1RpcTransactionWithSignature},
    },
    BlockEnv, HaltReason, Hardfork, L1SignedTransaction, TypedEnvelope,
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

impl ChainEvmSpec for L1ChainSpec {
    type PrecompileProvider<DatabaseT: Database> = EthPrecompiles;

    fn dry_run<
        DatabaseT: Database,
        PrecompileProviderT: PrecompileProvider<ContextForChainSpec<Self, DatabaseT>, Output = InterpreterResult>,
    >(
        block: Self::BlockEnv,
        cfg: CfgEnv<Self::Hardfork>,
        transaction: Self::SignedTransaction,
        database: DatabaseT,
        precompile_provider: PrecompileProviderT,
    ) -> Result<
        revm_context::result::ResultAndState<Self::HaltReason>,
        TransactionError<
            DatabaseT::Error,
            <Self::SignedTransaction as TransactionValidation>::ValidationError,
        >,
    > {
        let context = revm_context::Context {
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
        DatabaseT: Database,
        InspectorT: Inspector<ContextForChainSpec<Self, DatabaseT>>,
        PrecompileProviderT: PrecompileProvider<ContextForChainSpec<Self, DatabaseT>, Output = InterpreterResult>,
    >(
        block: Self::BlockEnv,
        cfg: CfgEnv<Self::Hardfork>,
        transaction: Self::SignedTransaction,
        database: DatabaseT,
        precompile_provider: PrecompileProviderT,
        inspector: InspectorT,
    ) -> Result<
        revm_context::result::ResultAndState<Self::HaltReason>,
        TransactionError<
            DatabaseT::Error,
            <Self::SignedTransaction as TransactionValidation>::ValidationError,
        >,
    > {
        let context = revm_context::Context {
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

impl ChainHardfork for L1ChainSpec {
    type Hardfork = Hardfork;
}

impl ChainReceiptSpec for L1ChainSpec {
    type Receipt = L1BlockReceipt<<Self as ExecutionReceiptChainSpec>::ExecutionReceipt<FilterLog>>;
}

impl ChainContextSpec for L1ChainSpec {
    type Context = ();
}

impl ChainRpcBlock for L1ChainSpec {
    type RpcBlock<DataT>
        = L1RpcBlock<DataT>
    where
        DataT: Default + DeserializeOwned + Serialize;
}

impl ChainSpec for L1ChainSpec {
    type BlockEnv = BlockEnv;
    type HaltReason = HaltReason;
    type SignedTransaction = L1SignedTransaction;
}

impl GenesisBlockFactory for L1ChainSpec {
    type CreationError = LocalBlockCreationError;

    type LocalBlock = EthLocalBlock<
        <Self as ChainReceiptSpec>::Receipt,
        Self,
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

impl RpcSpec for L1ChainSpec {
    type RpcCallRequest = L1CallRequest;
    type RpcReceipt = L1RpcTransactionReceipt;
    type RpcTransaction = L1RpcTransactionWithSignature;
    type RpcTransactionRequest = L1RpcTransactionRequest;
}
