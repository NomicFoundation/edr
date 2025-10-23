use edr_block_header::BlockHeader;
use edr_blockchain_api::BlockHashByNumber;
use edr_chain_spec::{
    BlobExcessGasAndPrice, BlockEnvConstructor, EvmTransactionValidationError,
    TransactionValidation,
};
use edr_database_components::{DatabaseComponents, WrapDatabaseRef};
use edr_evm2::guaranteed_dry_run_with_inspector;
use edr_evm_spec::{
    config::EvmConfig, result::ExecutionResult, BlockEnvTrait, ContextForChainSpec, Inspector,
};
use edr_precompile::PrecompileFn;
use edr_primitives::{Address, HashMap, B256, U256};
use edr_state_api::{State, StateError};

use crate::{
    error::ProviderErrorForChainSpec, time::TimeSinceEpoch, ProviderError, SyncProviderSpec,
};

struct BlockEnvWithZeroBaseFee<BlockEnvT: BlockEnvTrait> {
    inner: BlockEnvT,
}

impl<BlockEnvT: BlockEnvTrait> BlockEnvTrait for BlockEnvWithZeroBaseFee<BlockEnvT> {
    fn number(&self) -> U256 {
        self.inner.number()
    }

    fn beneficiary(&self) -> Address {
        self.inner.beneficiary()
    }

    fn timestamp(&self) -> U256 {
        self.inner.timestamp()
    }

    fn gas_limit(&self) -> u64 {
        self.inner.gas_limit()
    }

    fn basefee(&self) -> u64 {
        // `eth_call` uses a base fee of zero to mimick geth's behavior
        self.inner.basefee().map(|_| 0)
    }

    fn difficulty(&self) -> U256 {
        self.inner.difficulty()
    }

    fn prevrandao(&self) -> Option<B256> {
        self.inner.prevrandao()
    }

    fn blob_excess_gas_and_price(&self) -> Option<BlobExcessGasAndPrice> {
        self.inner.blob_excess_gas_and_price()
    }
}

/// Execute a transaction as a call. Returns the gas used and the output.
pub(super) fn run_call<'builder, BlockchainT, ChainSpecT, InspectorT, StateT, TimerT>(
    blockchain: BlockchainT,
    state: StateT,
    evm_config: EvmConfig,
    block_header: &'builder BlockHeader,
    block_hardfork: ChainSpecT::Hardfork,
    transaction: ChainSpecT::SignedTransaction,
    custom_precompiles: &HashMap<Address, PrecompileFn>,
    inspector: &mut InspectorT,
) -> Result<ExecutionResult<ChainSpecT::HaltReason>, ProviderErrorForChainSpec<ChainSpecT>>
where
    BlockchainT: BlockHashByNumber<Error = BlockchainErrorForChainSpec<ChainSpecT>>,
    ChainSpecT: SyncProviderSpec<
        TimerT,
        SignedTransaction: Default
                               + TransactionValidation<
            ValidationError: From<EvmTransactionValidationError>,
        >,
    >,
    InspectorT: Inspector<
        ContextForChainSpec<
            ChainSpecT,
            BlockEnvWithZeroBaseFee<ChainSpecT::BlockEnv<'builder, BlockHeader>>,
            WrapDatabaseRef<DatabaseComponents<BlockchainT, StateT>>,
        >,
    >,
    StateT: State<Error = StateError>,
    TimerT: Clone + TimeSinceEpoch,
{
    let block_env = ChainSpecT::BlockEnv::new_block_env(block_header, block_hardfork);

    guaranteed_dry_run_with_inspector::<_, ChainSpecT, _, _>(
        blockchain,
        state,
        evm_config.to_cfg_env(block_hardfork),
        transaction,
        block_env,
        custom_precompiles,
        inspector,
    )
    .map_or_else(
        |error| Err(ProviderError::RunTransaction(error)),
        |result| Ok(result.result),
    )
}
