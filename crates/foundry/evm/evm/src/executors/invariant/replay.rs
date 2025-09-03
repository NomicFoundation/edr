use std::{collections::HashMap, sync::Arc};

use alloy_dyn_abi::JsonAbiExt;
use alloy_primitives::Log;
use derive_where::derive_where;
use edr_solidity::contract_decoder::NestedTraceDecoder;
use eyre::Result;
use foundry_evm_core::{
    contracts::{ContractsByAddress, ContractsByArtifact},
    decode::RevertDecoder,
    evm_context::{
        BlockEnvTr, ChainContextTr, EvmBuilderTrait, HardforkTr, TransactionEnvTr,
        TransactionErrorTrait,
    },
};
use foundry_evm_coverage::HitMaps;
use foundry_evm_fuzz::{
    invariant::{BasicTxDetails, InvariantContract},
    BaseCounterExample,
};
use foundry_evm_traces::{load_contracts, TraceKind, Traces, TracingMode};
use parking_lot::RwLock;
use proptest::test_runner::TestError;
use revm::{
    context::result::{HaltReason, HaltReasonTr},
    interpreter::InstructionResult,
    primitives::U256,
};

use super::{
    call_after_invariant_function, call_invariant_function, error::FailedInvariantCaseData,
    shrink_sequence, CallAfterInvariantResult, CallInvariantResult,
};
use crate::executors::{
    stack_trace::{get_stack_trace, StackTraceResult},
    Executor,
};

/// Arguments to `replay_run`.
pub struct ReplayRunArgs<
    'a,
    NestedTraceDecoderT: NestedTraceDecoder<HaltReasonT>,
    BlockT: BlockEnvTr,
    TxT: TransactionEnvTr,
    EvmBuilderT: EvmBuilderTrait<BlockT, ChainContextT, HaltReasonT, HardforkT, TransactionErrorT, TxT>,
    HaltReasonT: HaltReasonTr,
    HardforkT: HardforkTr,
    TransactionErrorT: TransactionErrorTrait,
    ChainContextT: ChainContextTr,
> {
    pub executor: Executor<
        BlockT,
        TxT,
        EvmBuilderT,
        HaltReasonT,
        HardforkT,
        TransactionErrorT,
        ChainContextT,
    >,
    pub invariant_contract: &'a InvariantContract<'a>,
    pub known_contracts: &'a ContractsByArtifact,
    pub ided_contracts: ContractsByAddress,
    pub logs: &'a mut Vec<Log>,
    pub traces: &'a mut Traces,
    pub coverage: &'a mut Option<HitMaps>,
    pub deprecated_cheatcodes: &'a mut HashMap<&'static str, Option<&'static str>>,
    pub inputs: Vec<BasicTxDetails>,
    pub generate_stack_trace: bool,
    /// Must be provided if `generate_stack_trace` is true
    pub contract_decoder: Option<&'a NestedTraceDecoderT>,
    pub revert_decoder: &'a RevertDecoder,
    pub fail_on_revert: bool,
    pub show_solidity: bool,
}

/// Results of a replay
#[derive(Debug)]
#[derive_where(Default)]
pub struct ReplayResult<HaltReasonT: HaltReasonTr> {
    pub counterexample_sequence: Vec<BaseCounterExample>,
    pub stack_trace_result: Option<StackTraceResult<HaltReasonT>>,
    pub revert_reason: Option<String>,
}

/// Replays a call sequence for collecting logs and traces.
/// Returns counterexample to be used when the call sequence is a failed
/// scenario.
pub fn replay_run<
    NestedTraceDecoderT: NestedTraceDecoder<HaltReasonT>,
    BlockT: BlockEnvTr,
    TxT: TransactionEnvTr,
    EvmBuilderT: 'static
        + EvmBuilderTrait<BlockT, ChainContextT, HaltReasonT, HardforkT, TransactionErrorT, TxT>,
    HaltReasonT: 'static + HaltReasonTr + TryInto<HaltReason>,
    HardforkT: HardforkTr,
    TransactionErrorT: TransactionErrorTrait,
    ChainContextT: 'static + ChainContextTr,
>(
    args: ReplayRunArgs<
        '_,
        NestedTraceDecoderT,
        BlockT,
        TxT,
        EvmBuilderT,
        HaltReasonT,
        HardforkT,
        TransactionErrorT,
        ChainContextT,
    >,
) -> Result<ReplayResult<HaltReasonT>> {
    let ReplayRunArgs {
        mut executor,
        invariant_contract,
        known_contracts,
        mut ided_contracts,
        logs,
        traces,
        coverage,
        deprecated_cheatcodes,
        inputs,
        generate_stack_trace,
        contract_decoder,
        revert_decoder,
        fail_on_revert,
        show_solidity,
    } = args;

    // We want traces for a failed case.

    executor.set_tracing(if generate_stack_trace && executor.safe_to_re_execute() {
        TracingMode::WithSteps
    } else {
        TracingMode::WithoutSteps
    });

    let mut counterexample_sequence = vec![];

    // Replay each call from the sequence, collect logs, traces and coverage.
    for tx in inputs.iter() {
        let call_result = executor.call_raw_committing(
            tx.sender,
            tx.call_details.target,
            tx.call_details.calldata.clone(),
            U256::ZERO,
        )?;
        logs.extend(call_result.logs);
        traces.push((
            TraceKind::Execution,
            call_result.traces.clone().expect("enabled tracing"),
        ));
        HitMaps::merge_opt(coverage, call_result.coverage);

        // Identify newly generated contracts, if they exist.
        ided_contracts.extend(load_contracts(
            call_result.traces.iter().map(|a| &a.arena),
            known_contracts,
        ));

        // Create counter example to be used in failed case.
        counterexample_sequence.push(BaseCounterExample::from_invariant_call(
            tx.sender,
            tx.call_details.target,
            &tx.call_details.calldata,
            &ided_contracts,
            call_result.traces,
            show_solidity,
            /* indeterminism_reason */ None,
        ));

        // If this call failed, but didn't revert, this is terminal for sure.
        // If this call reverted, only exit if `fail_on_revert` is true.
        if !call_result
            .exit_reason
            .is_some_and(InstructionResult::is_ok)
            && (fail_on_revert || !call_result.reverted)
        {
            let stack_trace_result =
                if let Some(indeterminism_reasons) = executor.indeterminism_reasons() {
                    Some(indeterminism_reasons.into())
                } else {
                    contract_decoder
                        .and_then(|decoder| get_stack_trace(decoder, traces).transpose())
                        .map(StackTraceResult::from)
                };
            let revert_reason =
                revert_decoder.maybe_decode(call_result.result.as_ref(), call_result.exit_reason);
            return Ok(ReplayResult {
                counterexample_sequence,
                stack_trace_result,
                revert_reason,
            });
        }
    }

    // Replay invariant to collect logs and traces.
    // We do this only once at the end of the replayed sequence.
    // Checking after each call doesn't add valuable info for passing scenario
    // (invariant call result is always success) nor for failed scenarios
    // (invariant call result is always success until the last call that breaks it).
    let CallInvariantResult {
        call_result: invariant_result,
        success: invariant_success,
        cow_backend,
    } = call_invariant_function(
        &executor,
        invariant_contract.address,
        invariant_contract
            .invariant_function
            .abi_encode_input(&[])?
            .into(),
    )?;

    traces.push((
        TraceKind::Execution,
        invariant_result.traces.expect("tracing is on"),
    ));
    logs.extend(invariant_result.logs);
    deprecated_cheatcodes.extend(
        invariant_result
            .cheatcodes
            .as_ref()
            .map_or_else(Default::default, |cheats| cheats.deprecated.clone()),
    );

    // Collect after invariant logs and traces.
    if invariant_contract.call_after_invariant && invariant_success {
        let CallAfterInvariantResult {
            call_result: after_invariant_result,
            success: _,
        } = call_after_invariant_function(&executor, invariant_contract.address)?;
        traces.push((
            TraceKind::Execution,
            after_invariant_result.traces.clone().unwrap(),
        ));
        logs.extend(after_invariant_result.logs);
    }

    let stack_trace_result: Option<StackTraceResult<HaltReasonT>> =
        if let Some(indeterminism_reasons) = cow_backend.backend.indeterminism_reasons() {
            Some(indeterminism_reasons.into())
        } else {
            contract_decoder
                .and_then(|decoder| get_stack_trace(decoder, traces).transpose())
                .map(StackTraceResult::from)
        };

    let revert_reason = revert_decoder.maybe_decode(
        invariant_result.result.as_ref(),
        invariant_result.exit_reason,
    );

    Ok(ReplayResult {
        counterexample_sequence,
        stack_trace_result,
        revert_reason,
    })
}

/// Arguments to `replay_run`.
pub struct ReplayErrorArgs<
    'a,
    NestedTraceDecoderT,
    BlockT: BlockEnvTr,
    TxT: TransactionEnvTr,
    EvmBuilderT: EvmBuilderTrait<BlockT, ChainContextT, HaltReasonT, HardforkT, TransactionErrorT, TxT>,
    HaltReasonT: HaltReasonTr,
    HardforkT: HardforkTr,
    TransactionErrorT: TransactionErrorTrait,
    ChainContextT: ChainContextTr,
> {
    pub executor: Executor<
        BlockT,
        TxT,
        EvmBuilderT,
        HaltReasonT,
        HardforkT,
        TransactionErrorT,
        ChainContextT,
    >,
    pub failed_case: &'a FailedInvariantCaseData,
    pub invariant_contract: &'a InvariantContract<'a>,
    pub known_contracts: &'a ContractsByArtifact,
    pub ided_contracts: ContractsByAddress,
    pub logs: &'a mut Vec<Log>,
    pub traces: &'a mut Traces,
    pub coverage: &'a mut Option<HitMaps>,
    pub deprecated_cheatcodes: &'a mut HashMap<&'static str, Option<&'static str>>,
    pub generate_stack_trace: bool,
    /// Must be provided if `generate_stack_trace` is true
    pub contract_decoder: Option<&'a NestedTraceDecoderT>,
    pub revert_decoder: &'a RevertDecoder,
    pub show_solidity: bool,
}

/// Replays the error case, shrinks the failing sequence and collects all
/// necessary traces.
pub fn replay_error<
    NestedTraceDecoderT: NestedTraceDecoder<HaltReasonT>,
    BlockT: BlockEnvTr,
    TxT: TransactionEnvTr,
    EvmBuilderT: 'static
        + EvmBuilderTrait<BlockT, ChainContextT, HaltReasonT, HardforkT, TransactionErrorT, TxT>,
    HaltReasonT: 'static + HaltReasonTr + TryInto<HaltReason>,
    HardforkT: HardforkTr,
    TransactionErrorT: TransactionErrorTrait,
    ChainContextT: 'static + ChainContextTr,
>(
    args: ReplayErrorArgs<
        '_,
        NestedTraceDecoderT,
        BlockT,
        TxT,
        EvmBuilderT,
        HaltReasonT,
        HardforkT,
        TransactionErrorT,
        ChainContextT,
    >,
) -> Result<ReplayResult<HaltReasonT>> {
    let ReplayErrorArgs {
        mut executor,
        failed_case,
        invariant_contract,
        known_contracts,
        ided_contracts,
        logs,
        traces,
        coverage,
        deprecated_cheatcodes,
        generate_stack_trace,
        contract_decoder,
        revert_decoder,
        show_solidity,
    } = args;

    match failed_case.test_error {
        // Don't use at the moment.
        TestError::Abort(_) => Ok(ReplayResult::default()),
        TestError::Fail(_, ref calls) => {
            // Shrink sequence of failed calls.
            let calls = shrink_sequence(
                failed_case,
                calls,
                &executor,
                invariant_contract.call_after_invariant,
            )?;

            set_up_inner_replay(&mut executor, &failed_case.inner_sequence);

            // Replay calls to get the counterexample and to collect logs, traces and
            // coverage.
            replay_run(ReplayRunArgs {
                invariant_contract,
                executor,
                known_contracts,
                ided_contracts,
                logs,
                traces,
                coverage,
                deprecated_cheatcodes,
                inputs: calls,
                generate_stack_trace,
                contract_decoder,
                fail_on_revert: failed_case.fail_on_revert,
                revert_decoder,
                show_solidity,
            })
        }
    }
}

/// Sets up the calls generated by the internal fuzzer, if they exist.
fn set_up_inner_replay<
    BlockT: BlockEnvTr,
    TxT: TransactionEnvTr,
    EvmBuilderT: EvmBuilderTrait<BlockT, ChainContextT, HaltReasonT, HardforkT, TransactionErrorT, TxT>,
    HaltReasonT: HaltReasonTr,
    HardforkT: HardforkTr,
    TransactionErrorT: TransactionErrorTrait,
    ChainContextT: ChainContextTr,
>(
    executor: &mut Executor<
        BlockT,
        TxT,
        EvmBuilderT,
        HaltReasonT,
        HardforkT,
        TransactionErrorT,
        ChainContextT,
    >,
    inner_sequence: &[Option<BasicTxDetails>],
) {
    if let Some(fuzzer) = &mut executor.inspector.fuzzer {
        if let Some(call_generator) = &mut fuzzer.call_generator {
            call_generator.last_sequence = Arc::new(RwLock::new(inner_sequence.to_owned()));
            call_generator.set_replay(true);
        }
    }
}
