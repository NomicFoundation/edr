use edr_block_header::BlockHeader;
use edr_blockchain_api::{r#dyn::DynBlockchainError, BlockHashByNumber};
use edr_chain_spec::BlobExcessGasAndPrice;
use edr_chain_spec_evm::{BlockEnvTrait, CfgEnv};
use edr_chain_spec_provider::ProviderChainSpec;
use edr_evm::{set_guarantees, ExecutionResultWithMetadata};
use edr_inspection::{InspectedExecution, InspectionError, InspectionRequest};
use edr_precompile::PrecompileFn;
use edr_primitives::{Address, HashMap, B256, U256};
use edr_state_api::{State, StateError};

use crate::{
    error::ProviderErrorForChainSpec,
    observability::{EvmObserverConfig, ObservedExecution},
    ProviderError,
};

/// A wrapper around a block environment that forces the base fee to be zero.
///
/// This is required to mimick Geth's behaviour, as its call requests use a base
/// fee of zero.
pub(super) struct BlockEnvWithZeroBaseFee<BlockEnvT: BlockEnvTrait> {
    inner: BlockEnvT,
}

impl<BlockEnvT: BlockEnvTrait> BlockEnvWithZeroBaseFee<BlockEnvT> {
    /// Creates a new instance wrapping the provided block environment.
    pub fn new(inner: BlockEnvT) -> Self {
        Self { inner }
    }
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

    fn slot_num(&self) -> u64 {
        self.inner.slot_num()
    }
}

/// Executes a transaction as a call, observed as described by
/// `observer_config`.
pub(super) fn run_call<'call, ChainSpecT, BlockchainT, StateT>(
    blockchain: BlockchainT,
    block_env: ChainSpecT::BlockEnv<'call, BlockHeader>,
    state: StateT,
    mut cfg_env: CfgEnv<ChainSpecT::ProtocolHardfork>,
    transaction: ChainSpecT::SignedTransaction,
    custom_precompiles: &'call HashMap<Address, PrecompileFn>,
    observer_config: &EvmObserverConfig,
) -> Result<
    ObservedExecution<ExecutionResultWithMetadata<ChainSpecT::HaltReason>>,
    ProviderErrorForChainSpec<ChainSpecT>,
>
where
    BlockchainT: BlockHashByNumber<Error = DynBlockchainError>,
    ChainSpecT: ProviderChainSpec,
    StateT: State<Error = StateError>,
{
    set_guarantees(&mut cfg_env);

    let request = InspectionRequest {
        observer_config: observer_config.clone(),
        debug_trace: None,
    };

    let InspectedExecution {
        execution,
        observed,
        geth_trace: _,
    } = ChainSpecT::dry_run_with_inspection(
        &blockchain,
        &state,
        cfg_env,
        transaction,
        BlockEnvWithZeroBaseFee { inner: block_env },
        custom_precompiles,
        request,
    )
    .map_err(|error| match error {
        InspectionError::Transaction(error) => ProviderError::RunTransaction(error),
        InspectionError::Collection(error) => ProviderError::from(error),
        InspectionError::Database(_) | InspectionError::DebugTracer(_) => {
            unreachable!("only the debug tracer fails with these, and none was requested")
        }
    })?;

    Ok(ObservedExecution {
        evm_observed_data: observed,
        execution_result: execution.into_result_with_metadata(),
    })
}
