use alloy_rlp::RlpEncodable;
use edr_block_api::{GenesisBlockFactory, GenesisBlockOptions};
use edr_block_header::BlockConfig;
use edr_block_local::{EthLocalBlock, LocalBlockCreationError};
use edr_chain_spec::{ChainHardfork, ChainSpec};
use edr_evm_spec::{
    handler::{EthFrame, EthInstructions, EthPrecompiles},
    interpreter::{EthInterpreter, InterpreterResult},
    ChainEvmSpec, ContextForChainSpec, Database, DatabaseComponentError, Evm, Inspector,
    PrecompileProvider,
};
use edr_primitives::Bytes;
use edr_receipt::{log::FilterLog, ChainExecutionReceipt};
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

impl ChainEvmSpec for L1ChainSpec {
    type Evm<
        BlockchainErrorT,
        DatabaseT: Database<Error = DatabaseComponentError<BlockchainErrorT, StateErrorT>>,
        InspectorT: Inspector<ContextForChainSpec<Self, DatabaseT>>,
        PrecompileProviderT: PrecompileProvider<ContextForChainSpec<Self, DatabaseT>, Output = InterpreterResult>,
        StateErrorT,
    > = Evm<
        ContextForChainSpec<Self, DatabaseT>,
        InspectorT,
        EthInstructions<EthInterpreter, ContextForChainSpec<Self, DatabaseT>>,
        PrecompileProviderT,
        EthFrame<EthInterpreter>,
    >;

    type PrecompileProvider<
        BlockchainErrorT,
        DatabaseT: Database<Error = DatabaseComponentError<BlockchainErrorT, StateErrorT>>,
        StateErrorT,
    > = EthPrecompiles;

    fn evm_with_inspector<
        BlockchainErrorT,
        DatabaseT: Database<Error = DatabaseComponentError<BlockchainErrorT, StateErrorT>>,
        InspectorT: Inspector<ContextForChainSpec<Self, DatabaseT>>,
        PrecompileProviderT: PrecompileProvider<ContextForChainSpec<Self, DatabaseT>, Output = InterpreterResult>,
        StateErrorT,
    >(
        block: Self::BlockEnv,
        cfg: CfgEnv<Self::Hardfork>,
        transaction: Self::SignedTransaction,
        database: DatabaseT,
        inspector: InspectorT,
        precompile_provider: PrecompileProviderT,
    ) -> Result<
        Self::Evm<BlockchainErrorT, DatabaseT, InspectorT, PrecompileProviderT, StateErrorT>,
        DatabaseT::Error,
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

        Ok(Evm::new_with_inspector(
            context,
            inspector,
            EthInstructions::default(),
            precompile_provider,
        ))
    }
}

impl ChainExecutionReceipt for L1ChainSpec {
    type ExecutionReceipt<LogT> = TypedEnvelope<edr_receipt::execution::Eip658<LogT>>;
}

impl ChainHardfork for L1ChainSpec {
    type Hardfork = Hardfork;
}

impl ChainReceiptSpec for L1ChainSpec {
    type TransactionReceipt =
        L1BlockReceipt<<Self as ChainExecutionReceipt>::ExecutionReceipt<FilterLog>>;
}

impl ChainRpcBlock for L1ChainSpec {
    type RpcBlock<DataT>
        = L1RpcBlock<DataT>
    where
        DataT: Default + DeserializeOwned + Serialize;
}

impl ChainSpec for L1ChainSpec {
    type BlockEnv = BlockEnv;
    type Context = ();
    type HaltReason = HaltReason;
    type SignedTransaction = L1SignedTransaction;
}

impl GenesisBlockFactory for L1ChainSpec {
    type CreationError = LocalBlockCreationError;

    type LocalBlock = EthLocalBlock<
        <Self as ChainReceiptSpec>::TransactionReceipt,
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
