use core::cmp;
use std::sync::Arc;

use edr_block_api::{Block as _, FetchBlockReceipts};
use edr_block_header::BlockHeader;
use edr_blockchain_api::{r#dyn::DynBlockchainError, BlockHashByNumber};
use edr_chain_spec::{
    BlockEnvChainSpec, BlockEnvConstructor as _, ChainSpec, ExecutableTransaction as _,
    HaltReasonTrait, HardforkChainSpec,
};
use edr_chain_spec_evm::{
    interpreter::InstructionResult,
    result::{ExecutionResult, ResultGas},
    CfgEnv,
};
use edr_chain_spec_provider::ProviderChainSpec;
use edr_eip7892::ScheduledBlobParams;
use edr_eth::reward_percentile::RewardPercentile;
use edr_evm::ExecutionResultWithMetadata;
use edr_precompile::PrecompileFn;
use edr_primitives::{Address, HashMap, U256};
use edr_receipt::ReceiptTrait as _;
use edr_solidity::contract_decoder::ContractDecoder;
use edr_state_api::DynState;
use edr_transaction::TransactionMut;
use foundry_evm_traces::CallTraceArena;
use itertools::Itertools;
use parking_lot::RwLock;

use crate::{
    config::GasEstimationMode,
    data::call,
    error::{EstimateGasFailure, ProviderErrorForChainSpec, TransactionFailure},
    observability::{
        observe_execution, EvmObservedData, EvmObserver, EvmObserverConfig, ObservedExecution,
    },
    time::TimeSinceEpoch,
    ProviderError, ProviderSpec, TransactionFailureReason,
};

pub struct EstimateGasResult {
    pub call_trace_arenas: Vec<CallTraceArena>,
    pub estimation: u64,
}

/// Shared EVM execution context passed to all gas estimation functions.
pub(super) struct GasCallContext<'a, ChainSpecT: ChainSpec + HardforkChainSpec> {
    pub blockchain: &'a dyn BlockHashByNumber<Error = DynBlockchainError>,
    pub cfg_env: CfgEnv<ChainSpecT::Hardfork>,
    pub custom_precompiles: &'a HashMap<Address, PrecompileFn>,
    pub header: &'a BlockHeader,
    pub scheduled_blob_params: Option<&'a ScheduledBlobParams>,
    pub state: &'a dyn DynState,
    pub transaction: ChainSpecT::SignedTransaction,
}

impl GasEstimationMode {
    pub(crate) fn accepts<HaltReasonT>(
        &self,
        result: &ExecutionResult<HaltReasonT>,
        traces: &CallTraceArena,
    ) -> bool {
        match self {
            GasEstimationMode::TopLevelSuccess => result.is_success(),
            GasEstimationMode::NoInternalOutOfGas => {
                result.is_success() && !has_internal_oog(traces)
            }
        }
    }

    /// Returns the gas used by an execution that the EVM reported as
    /// successful, provided this mode accepts it; otherwise returns the
    /// mode-specific failure explaining the rejection.
    #[allow(clippy::result_large_err)]
    pub(crate) fn accepted_gas_used<HaltReasonT: HaltReasonTrait>(
        &self,
        gas: ResultGas,
        evm_observed_data: &EvmObservedData,
        contract_decoder: &RwLock<ContractDecoder>,
    ) -> Result<u64, TransactionFailure<HaltReasonT>> {
        match self {
            GasEstimationMode::TopLevelSuccess => Ok(gas.tx_gas_used()),
            GasEstimationMode::NoInternalOutOfGas => {
                if has_internal_oog(&evm_observed_data.call_trace_arena) {
                    Err(TransactionFailure::halt(
                        TransactionFailureReason::InternalCallOutOfGas,
                        None,
                        &evm_observed_data.address_to_executed_code,
                        &evm_observed_data.call_trace_arena,
                        contract_decoder,
                    ))
                } else {
                    Ok(gas.tx_gas_used())
                }
            }
        }
    }
}

/// Returns true if any sub-call (non-root) in the trace arena ended in an
/// out-of-gas variant. The OOG variants mirror REVM's
/// `InstructionResult::is_out_of_gas` macro.
fn has_internal_oog(arena: &CallTraceArena) -> bool {
    arena.nodes().iter().any(|node| {
        // Skip the root call — only sub-calls count as "internal".
        node.parent.is_some()
            && matches!(
                node.trace.status,
                Some(
                    InstructionResult::OutOfGas
                        | InstructionResult::MemoryOOG
                        | InstructionResult::MemoryLimitOOG
                        | InstructionResult::PrecompileOOG
                        | InstructionResult::InvalidOperandOOG
                        | InstructionResult::ReentrancySentryOOG
                )
            )
    })
}

impl<'a, ChainSpecT: ChainSpec + HardforkChainSpec + BlockEnvChainSpec>
    GasCallContext<'a, ChainSpecT>
{
    fn new_block_env(&self) -> ChainSpecT::BlockEnv<'_, BlockHeader> {
        ChainSpecT::BlockEnv::new_block_env(
            self.header,
            self.cfg_env.spec,
            self.scheduled_blob_params,
        )
    }
}

/// Executes the transaction with the given gas limit and returns the full
/// execution result.
fn run_with_gas_limit<ChainSpecT: ProviderChainSpec<SignedTransaction: TransactionMut>>(
    context: &GasCallContext<'_, ChainSpecT>,
    gas_limit: u64,
    observer: &mut EvmObserver,
) -> Result<
    ExecutionResultWithMetadata<ChainSpecT::HaltReason>,
    ProviderErrorForChainSpec<ChainSpecT>,
> {
    let mut transaction = context.transaction.clone();
    transaction.set_gas_limit(gas_limit);
    call::run_call::<ChainSpecT, _, _, _>(
        context.blockchain,
        context.new_block_env(),
        context.state,
        context.cfg_env.clone(),
        transaction,
        context.custom_precompiles,
        observer,
    )
}

/// Searches for the lowest gas limit that `estimation_mode` accepts, between
/// `lower_bound` and `upper_bound`. Matches Hardhat logic, except it's
/// iterative, not recursive.
///
/// The caller must guarantee that the initial `upper_bound` is accepted by
/// `estimation_mode`, so that the returned estimation is always backed by an
/// accepted execution.
fn binary_search_estimation<ChainSpecT: ProviderChainSpec<SignedTransaction: TransactionMut>>(
    context: &GasCallContext<'_, ChainSpecT>,
    mut lower_bound: u64,
    mut upper_bound: u64,
    observer_config: &EvmObserverConfig,
    estimation_mode: GasEstimationMode,
) -> Result<EstimateGasResult, ProviderErrorForChainSpec<ChainSpecT>> {
    const MAX_ITERATIONS: usize = 20;

    let mut i = 0;
    let mut call_trace_arenas = Vec::new();

    while upper_bound - lower_bound > min_difference(lower_bound) && i < MAX_ITERATIONS {
        let mut mid = lower_bound + (upper_bound - lower_bound) / 2;
        if i == 0 {
            // Start close to the lower bound as it's assumed to be derived from the gas
            // used by the transaction.
            let initial_mid = 3 * lower_bound;
            mid = cmp::min(mid, initial_mid);
        }

        let (call_succeeded, call_trace) =
            probe_gas_limit(context, observer_config, mid, estimation_mode)?;

        call_trace_arenas.extend(call_trace);

        if call_succeeded {
            upper_bound = mid;
        } else {
            lower_bound = mid + 1;
        }

        i += 1;
    }

    Ok(EstimateGasResult {
        call_trace_arenas,
        estimation: upper_bound,
    })
}

/// Estimate the gas cost of a transaction. Matches Hardhat behavior.
pub(super) fn estimate_gas<
    TimerT: Clone + TimeSinceEpoch,
    ChainSpecT: ProviderSpec<TimerT, SignedTransaction: TransactionMut>,
>(
    context: &GasCallContext<'_, ChainSpecT>,
    contract_decoder: Arc<RwLock<ContractDecoder>>,
    minimum_cost: u64,
    observer_config: &EvmObserverConfig,
    estimation_mode: GasEstimationMode,
) -> Result<EstimateGasResult, ProviderErrorForChainSpec<ChainSpecT>> {
    let (gas_used, call_trace_arena) = measure_gas_with_full_limit(
        context,
        observer_config,
        estimation_mode,
        contract_decoder.as_ref(),
    )?;

    let mut call_trace_arenas: Vec<_> = call_trace_arena.into_iter().collect();

    // Ensure that the initial estimation is at least the minimum cost + 1.
    let lowest_estimation = cmp::max(gas_used, minimum_cost + 1);

    let (successful_execution, execution_trace) =
        probe_gas_limit(context, observer_config, lowest_estimation, estimation_mode)?;

    call_trace_arenas.extend(execution_trace);

    // Return the initial estimation if it was successful
    if successful_execution {
        return Ok(EstimateGasResult {
            estimation: lowest_estimation,
            call_trace_arenas,
        });
    }

    // The estimation mode didn't accept `lowest_estimation` as the gas limit.
    // This can happen if the execution behavior depends on the available gas.
    // Search upwards for the lowest gas limit the mode accepts.
    let EstimateGasResult {
        call_trace_arenas: estimation_call_trace_arenas,
        estimation,
    } = binary_search_estimation::<ChainSpecT>(
        context,
        lowest_estimation,
        // transaction's `gas_limit` was validated by `measure_gas_with_full_limit`
        context.transaction.gas_limit(),
        observer_config,
        estimation_mode,
    )?;

    call_trace_arenas.extend(estimation_call_trace_arenas);

    Ok(EstimateGasResult {
        call_trace_arenas,
        estimation,
    })
}

/// Measures the gas used by the transaction executed with its full gas limit,
/// reporting failures as if they came from `eth_call`.
///
/// Returns the gas used by the execution and its call trace (when trace
/// collection is enabled), or an [`EstimateGasFailure`] when the execution
/// fails or the estimation mode rejects it.
fn measure_gas_with_full_limit<
    TimerT: Clone + TimeSinceEpoch,
    ChainSpecT: ProviderSpec<TimerT, SignedTransaction: TransactionMut>,
>(
    context: &GasCallContext<'_, ChainSpecT>,
    observer_config: &EvmObserverConfig,
    estimation_mode: GasEstimationMode,
    contract_decoder: &RwLock<ContractDecoder>,
) -> Result<(u64, Option<CallTraceArena>), ProviderErrorForChainSpec<ChainSpecT>> {
    let ObservedExecution {
        evm_observed_data,
        execution_result,
        ..
    } = observe_execution(observer_config, |observer| {
        call::run_call::<'_, ChainSpecT, _, _, _>(
            context.blockchain,
            context.new_block_env(),
            context.state,
            context.cfg_env.clone(),
            context.transaction.clone(),
            context.custom_precompiles,
            observer,
        )
    })?;

    let initial_gas_or_failure = match execution_result.result {
        ExecutionResult::Success { gas, .. } => {
            estimation_mode.accepted_gas_used(gas, &evm_observed_data, contract_decoder)
        }
        ExecutionResult::Revert { output, .. } => Err(TransactionFailure::revert(
            output,
            None,
            &evm_observed_data.address_to_executed_code,
            &evm_observed_data.call_trace_arena,
            contract_decoder,
        )),
        ExecutionResult::Halt { reason, .. } => Err(TransactionFailure::halt(
            ChainSpecT::cast_halt_reason(reason),
            None,
            &evm_observed_data.address_to_executed_code,
            &evm_observed_data.call_trace_arena,
            contract_decoder,
        )),
    };

    let gas_used = match initial_gas_or_failure {
        Ok(gas_used) => gas_used,
        Err(transaction_failure) => {
            return Err(Box::new(EstimateGasFailure {
                address_to_executed_code: evm_observed_data.address_to_executed_code,
                call_trace_arena: evm_observed_data.call_trace_arena,
                encoded_console_logs: evm_observed_data.encoded_console_logs,
                precompile_addresses: execution_result.precompile_addresses,
                transaction_failure,
            })
            .into())
        }
    };
    let call_trace_arena =
        evm_observed_data.into_call_traces(observer_config.include_call_traces, true);
    Ok((gas_used, call_trace_arena))
}

/// Executes the transaction with the given gas limit and returns whether the
/// estimation mode accepts the execution, along with its call trace (when the
/// trace collection criteria are met).
fn probe_gas_limit<ChainSpecT: ProviderChainSpec<SignedTransaction: TransactionMut>>(
    context: &GasCallContext<'_, ChainSpecT>,
    observer_config: &EvmObserverConfig,
    gas_limit: u64,
    estimation_mode: GasEstimationMode,
) -> Result<(bool, Option<CallTraceArena>), ProviderErrorForChainSpec<ChainSpecT>> {
    let observed_limited_execution = observe_execution(observer_config, |observer| {
        run_with_gas_limit(context, gas_limit, observer)
    })?;

    let successful_execution = estimation_mode.accepts(
        observed_limited_execution.result(),
        &observed_limited_execution
            .evm_observed_data
            .call_trace_arena,
    );
    let should_include_traces = observer_config
        .include_call_traces
        .should_include(|| !successful_execution);

    let trace_arena = should_include_traces.then_some(
        observed_limited_execution
            .evm_observed_data
            .call_trace_arena,
    );

    Ok((successful_execution, trace_arena))
}

// Matches Hardhat
#[inline]
fn min_difference(lower_bound: u64) -> u64 {
    if lower_bound >= 4_000_000 {
        50_000
    } else if lower_bound >= 1_000_000 {
        10_000
    } else if lower_bound >= 100_000 {
        1_000
    } else if lower_bound >= 50_000 {
        500
    } else if lower_bound >= 30_000 {
        300
    } else {
        200
    }
}

/// Compute miner rewards for percentiles.
pub(super) fn compute_rewards<ChainSpecT: ProviderChainSpec>(
    block: &ChainSpecT::Block,
    reward_percentiles: &[RewardPercentile],
) -> Result<Vec<U256>, ProviderErrorForChainSpec<ChainSpecT>> {
    if block.transactions().is_empty() {
        return Ok(reward_percentiles.iter().map(|_| U256::ZERO).collect());
    }

    let base_fee_per_gas = block.block_header().base_fee_per_gas.unwrap_or_default();

    let gas_used_and_effective_reward = block
        .fetch_transaction_receipts()
        .map_err(ProviderError::FetchReceipt)?
        .iter()
        .enumerate()
        .map(|(i, receipt)| {
            let transaction = block
                .transactions()
                .get(i)
                .expect("receipt index should match transaction index");

            let gas_used = receipt.gas_used();
            // gas price pre EIP-1559 and max fee per gas post EIP-1559
            let gas_price = transaction.gas_price();

            let effective_reward =
                if let Some(max_priority_fee_per_gas) = transaction.max_priority_fee_per_gas() {
                    cmp::min(*max_priority_fee_per_gas, gas_price - base_fee_per_gas)
                } else {
                    gas_price.saturating_sub(base_fee_per_gas)
                };

            (gas_used, effective_reward)
        })
        .sorted_by(|(_, reward_first), (_, reward_second)| reward_first.cmp(reward_second))
        .collect::<Vec<(_, _)>>();

    // Ethereum block gas limit is 30 million, so it's safe to cast to f64.
    let gas_limit = block.block_header().gas_limit as f64;

    Ok(reward_percentiles
        .iter()
        .map(|percentile| {
            let mut gas_used = 0;
            let target_gas = ((percentile.as_ref() / 100.0) * gas_limit) as u64;

            for (gas_used_by_tx, effective_reward) in &gas_used_and_effective_reward {
                gas_used += gas_used_by_tx;
                if target_gas <= gas_used {
                    return U256::from(*effective_reward);
                }
            }

            gas_used_and_effective_reward
                .last()
                .map_or(U256::ZERO, |(_, reward)| U256::from(*reward))
        })
        .collect())
}

/// Gas used to gas limit ratio
pub(super) fn gas_used_ratio(gas_used: u64, gas_limit: u64) -> f64 {
    // Ported from Hardhat
    // https://github.com/NomicFoundation/hardhat/blob/0c547784952d6409e157b03ae69ba456b03cf6ee/packages/hardhat-core/src/internal/hardhat-network/provider/node.ts#L1359
    const FLOATS_PRECISION: f64 = 100_000.0;
    gas_used as f64 * FLOATS_PRECISION / gas_limit as f64 / FLOATS_PRECISION
}
