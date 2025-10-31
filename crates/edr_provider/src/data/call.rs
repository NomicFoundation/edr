use edr_block_header::BlockHeader;
use edr_blockchain_api::{r#dyn::DynBlockchainError, BlockHashByNumber};
use edr_chain_spec::{BlobExcessGasAndPrice, BlockEnvConstructor};
use edr_chain_spec_provider::ProviderChainSpec;
use edr_database_components::{DatabaseComponents, WrapDatabaseRef};
use edr_evm::guaranteed_dry_run_with_inspector;
use edr_evm_spec::{
    result::ExecutionResult, BlockEnvTrait, CfgEnv, ContextForChainSpec, Inspector,
};
use edr_precompile::PrecompileFn;
use edr_primitives::{Address, HashMap, B256, U256};
use edr_state_api::{State, StateError};

use crate::{error::ProviderErrorForChainSpec, ProviderError};

pub(super) struct BlockEnvWithZeroBaseFee<BlockEnvT: BlockEnvTrait> {
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
        0
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
pub(super) fn run_call<'call, ChainSpecT, BlockchainT, InspectorT, StateT>(
    blockchain: BlockchainT,
    block_header: &'call BlockHeader,
    state: StateT,
    cfg_env: CfgEnv<ChainSpecT::Hardfork>,
    transaction: ChainSpecT::SignedTransaction,
    custom_precompiles: &'call HashMap<Address, PrecompileFn>,
    inspector: &'call mut InspectorT,
) -> Result<ExecutionResult<ChainSpecT::HaltReason>, ProviderErrorForChainSpec<ChainSpecT>>
where
    BlockchainT: BlockHashByNumber<Error = DynBlockchainError>,
    ChainSpecT: ProviderChainSpec,
    InspectorT: Inspector<
        ContextForChainSpec<
            ChainSpecT,
            BlockEnvWithZeroBaseFee<ChainSpecT::BlockEnv<'call, BlockHeader>>,
            WrapDatabaseRef<DatabaseComponents<BlockchainT, StateT>>,
        >,
    >,
    StateT: State<Error = StateError>,
{
    let block_env = ChainSpecT::BlockEnv::new_block_env(block_header, cfg_env.spec);

    guaranteed_dry_run_with_inspector::<ChainSpecT, _, _, _, _>(
        blockchain,
        state,
        cfg_env,
        transaction,
        BlockEnvWithZeroBaseFee { inner: block_env },
        custom_precompiles,
        inspector,
    )
    .map_or_else(
        |error| Err(ProviderError::RunTransaction(error)),
        |result| Ok(result.result),
    )
}
