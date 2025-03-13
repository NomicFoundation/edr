use edr_eth::{block::Header, l1, result::ExecutionResult, transaction::TransactionValidation};
use edr_evm::{
    blockchain::{BlockHash, BlockchainErrorForChainSpec, SyncBlockchain},
    config::CfgEnv,
    evm::{Frame, FrameResult},
    interpreter::FrameInput,
    runtime::guaranteed_dry_run_with_extension,
    spec::{BlockEnvConstructor as _, ContextForChainSpec, SyncRuntimeSpec},
    state::{
        DatabaseComponents, State, StateError, StateOverrides, StateRefOverrider, SyncState,
        WrapDatabaseRef,
    },
    transaction::TransactionError,
};

use crate::ProviderError;

/// Execute a transaction as a call. Returns the gas used and the output.
pub(super) fn run_call<
    'components,
    'context,
    'extension,
    BlockchainT,
    ChainSpecT,
    ExtensionT,
    FrameT,
    StateT,
>(
    blockchain: BlockchainT,
    header: &Header,
    state: StateT,
    cfg_env: CfgEnv<ChainSpecT::Hardfork>,
    transaction: ChainSpecT::SignedTransaction,
    extension: &'extension mut ContextExtension<ExtensionT, FrameT>,
) -> Result<ExecutionResult<ChainSpecT::HaltReason>, ProviderError<ChainSpecT>>
where
    'components: 'context,
    'extension: 'context,
    BlockchainT: BlockHash<Error = BlockchainErrorForChainSpec<ChainSpecT>> + 'components,
    ChainSpecT: SyncRuntimeSpec<
        BlockEnv: Default,
        SignedTransaction: Default
                               + TransactionValidation<ValidationError: From<l1::InvalidTransaction>>,
    >,
    FrameT: Frame<
        Context<'context> = ExtendedContext<
            'context,
            ContextForChainSpec<
                ChainSpecT,
                WrapDatabaseRef<DatabaseComponents<BlockchainT, StateT>>,
            >,
            ExtensionT,
        >,
        Error = TransactionError<BlockchainErrorForChainSpec<ChainSpecT>, ChainSpecT, StateError>,
        FrameInit = FrameInput,
        FrameResult = FrameResult,
    >,
    StateT: State<Error = StateError> + 'components,
{
    // `eth_call` uses a base fee of zero to mimick geth's behavior
    let mut header = header.clone();
    header.base_fee_per_gas = header.base_fee_per_gas.map(|_| 0);

    let block = ChainSpecT::BlockEnv::new_block_env(&header, cfg_env.spec.into());

    guaranteed_dry_run_with_extension(blockchain, state, cfg_env, transaction, block, extension)
        .map_or_else(
            |error| Err(ProviderError::RunTransaction(error)),
            |result| Ok(result.result),
        )
}
