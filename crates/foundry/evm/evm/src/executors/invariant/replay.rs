use std::{collections::HashMap, sync::Arc};

use alloy_dyn_abi::JsonAbiExt;
use alloy_primitives::{Bytes, Log};
use derive_where::derive_where;
use edr_decoder_revert::RevertDecoder;
use edr_solidity::{
    contract_decoder::NestedTraceDecoder,
    solidity_stack_trace::{get_stack_trace, DeployedCode},
};
use eyre::Result;
use foundry_evm_core::{
    contracts::{ContractsByAddress, ContractsByArtifact},
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
use foundry_evm_traces::{load_contracts, ExecutionTraces, TracingMode};
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
    stack_trace::{SolidityTestStackTraceError, SolidityTestStackTraceResult},
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
    pub execution_traces: &'a mut ExecutionTraces,
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
    /// The code of contracts deployed during setup, for stack-trace
    /// decoding. Only consulted when `generate_stack_trace` is true.
    pub deployed_code: DeployedCode<'a>,
    pub line_coverage: &'a mut Option<HitMaps>,
    pub deprecated_cheatcodes: &'a mut HashMap<&'static str, Option<&'static str>>,
    pub inputs: &'a [BasicTxDetails],
    /// Whether to compute a stack trace for the replayed failure. When false,
    /// [`ReplayResult::stack_trace_result`] is always `None`.
    pub generate_stack_trace: bool,
    /// Must be provided if `generate_stack_trace` is true
    pub contract_decoder: Option<&'a NestedTraceDecoderT>,
    pub revert_decoder: &'a RevertDecoder,
    pub fail_on_revert: bool,
    /// Whether the caller still consumes the replayed arenas and so wants
    /// them accumulated in [`Self::execution_traces`]. When false — and no
    /// stack trace needs them — they are dropped with the call results they
    /// came from. Only consulted when `generate_stack_trace` is false:
    /// stack-trace generation keeps the arenas regardless.
    pub retain_traces: bool,
}

/// Results of a replay
#[derive(Debug)]
#[derive_where(Default)]
pub struct ReplayResult<HaltReasonT: HaltReasonTr> {
    pub counterexample_sequence: Vec<BaseCounterExample>,
    pub stack_trace_result: Option<SolidityTestStackTraceResult<HaltReasonT>>,
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
        execution_traces,
        mut executor,
        invariant_contract,
        known_contracts,
        mut ided_contracts,
        logs,
        deployed_code,
        line_coverage: coverage,
        deprecated_cheatcodes,
        inputs,
        generate_stack_trace,
        contract_decoder,
        revert_decoder,
        fail_on_revert,
        retain_traces,
    } = args;

    executor.set_tracing(if generate_stack_trace && executor.safe_to_re_execute() {
        TracingMode::WithSteps
    } else {
        TracingMode::WithoutSteps
    });

    // Stack-trace generation needs the accumulated arenas — the last as the
    // failing trace, the earlier ones as code sources — so keep them even
    // when the caller won't consume them afterwards; the caller's retention
    // policy frees them once the test finishes.
    let keep_traces = retain_traces || generate_stack_trace;

    let mut counterexample_sequence = vec![];

    // Replay each call from the sequence, collect logs, traces and coverage.
    for tx in inputs.iter() {
        let mut call_result = executor.transact_raw(
            tx.sender,
            tx.call_details.target,
            tx.call_details.calldata.clone(),
            U256::ZERO,
        )?;
        logs.extend(call_result.logs);
        HitMaps::merge_opt(coverage, call_result.line_coverage);

        // Identify newly generated contracts, if they exist.
        ided_contracts.extend(load_contracts(
            call_result.traces.iter().map(|a| &a.arena),
            known_contracts,
        ));

        if keep_traces {
            execution_traces.push(call_result.traces.take().expect("enabled tracing"));
        }

        // Create counter example to be used in failed case.
        counterexample_sequence.push(BaseCounterExample::from_invariant_call(
            tx.sender,
            tx.call_details.target,
            &tx.call_details.calldata,
            &ided_contracts,
            /* indeterminism_reasons */ None,
        ));

        // If this call failed, but didn't revert, this is terminal for sure.
        // If this call reverted, only exit if `fail_on_revert` is true.
        if !call_result
            .exit_reason
            .is_some_and(InstructionResult::is_ok)
            && (fail_on_revert || !call_result.reverted)
        {
            let stack_trace_result = if !generate_stack_trace {
                // The caller wants no stack trace; without `keep_traces` the
                // arenas it would need were never accumulated.
                None
            } else if let Some(indeterminism_reasons) = call_result.indeterminism_reasons {
                Some(indeterminism_reasons.into())
            } else {
                contract_decoder.map(|decoder| {
                    let (failing_trace, prior_traces) = execution_traces
                        .split_last()
                        .expect("`generate_stack_trace` implies `keep_traces`");

                    get_stack_trace(
                        decoder,
                        &failing_trace.arena,
                        prior_traces.iter().map(|arena| &arena.arena),
                        deployed_code,
                    )
                    .map_err(SolidityTestStackTraceError::from)
                    .into()
                })
            };
            let revert_reason =
                revert_decoder.maybe_decode(call_result.result.as_ref(), call_result.exit_reason);
            return Ok(ReplayResult {
                counterexample_sequence,
                stack_trace_result,
                revert_reason,
            });
        }

        // This call is not the failing one, so its arena can only ever serve
        // as a code source: strip it now rather than when the next push
        // displaces it, so the tracer's in-flight arena is the only
        // step-laden one.
        if keep_traces {
            execution_traces.strip_last_steps();
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
    } = call_invariant_function(
        &executor,
        invariant_contract.address,
        invariant_contract
            .invariant_function
            .abi_encode_input(&[])?
            .into(),
    )?;

    if keep_traces {
        execution_traces.push(invariant_result.traces.expect("tracing is on"));
    }
    logs.extend(invariant_result.logs);
    deprecated_cheatcodes.extend(
        invariant_result
            .cheatcodes
            .as_ref()
            .map_or_else(Default::default, |cheats| cheats.deprecated.clone()),
    );

    // Collect after invariant logs and traces. When `afterInvariant()` is what
    // failed, its output and exit reason — not `invariant()`'s, which
    // succeeded — carry the revert reason of the reproduced failure.
    let mut after_invariant_failure: Option<(Bytes, Option<InstructionResult>)> = None;
    if invariant_contract.call_after_invariant && invariant_success {
        let CallAfterInvariantResult {
            call_result: after_invariant_result,
            success: after_invariant_success,
        } = call_after_invariant_function(&executor, invariant_contract.address)?;
        if keep_traces {
            execution_traces.push(after_invariant_result.traces.expect("tracing is on"));
        }
        if !after_invariant_success {
            after_invariant_failure = Some((
                after_invariant_result.result,
                after_invariant_result.exit_reason,
            ));
        }
        logs.extend(after_invariant_result.logs);
    }

    let stack_trace_result: Option<SolidityTestStackTraceResult<HaltReasonT>> =
        generate_stack_trace
            .then(|| {
                invariant_result
                    .indeterminism_reasons
                    .map(SolidityTestStackTraceResult::from)
                    .or_else(|| {
                        contract_decoder.map(|decoder| {
                            // The failing call is always the last one
                            // replayed — `afterInvariant()` when it ran,
                            // otherwise `invariant()` — so its arena is the
                            // one just pushed, and the only one still
                            // carrying steps.
                            let (failing_trace, prior_traces) = execution_traces
                                .split_last()
                                .expect("`generate_stack_trace` implies `keep_traces`");

                            get_stack_trace(
                                decoder,
                                &failing_trace.arena,
                                prior_traces.iter().map(|arena| &arena.arena),
                                deployed_code,
                            )
                            .map_err(SolidityTestStackTraceError::from)
                            .into()
                        })
                    })
            })
            .flatten();

    let (failing_output, failing_exit_reason) = match &after_invariant_failure {
        Some((output, exit_reason)) => (output, *exit_reason),
        None => (&invariant_result.result, invariant_result.exit_reason),
    };
    let revert_reason = revert_decoder.maybe_decode(failing_output.as_ref(), failing_exit_reason);

    Ok(ReplayResult {
        counterexample_sequence,
        stack_trace_result,
        revert_reason,
    })
}

/// Arguments to `replay_error`.
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
    pub execution_traces: &'a mut ExecutionTraces,
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
    /// See [`ReplayRunArgs::deployed_code`].
    pub deployed_code: DeployedCode<'a>,
    pub coverage: &'a mut Option<HitMaps>,
    pub deprecated_cheatcodes: &'a mut HashMap<&'static str, Option<&'static str>>,
    pub generate_stack_trace: bool,
    /// Must be provided if `generate_stack_trace` is true
    pub contract_decoder: Option<&'a NestedTraceDecoderT>,
    pub revert_decoder: &'a RevertDecoder,
    /// See [`ReplayRunArgs::retain_traces`].
    pub retain_traces: bool,
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
        execution_traces,
        mut executor,
        failed_case,
        invariant_contract,
        known_contracts,
        ided_contracts,
        logs,
        deployed_code,
        coverage,
        deprecated_cheatcodes,
        generate_stack_trace,
        contract_decoder,
        revert_decoder,
        retain_traces,
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
                execution_traces,
                invariant_contract,
                executor,
                known_contracts,
                ided_contracts,
                logs,
                deployed_code,
                line_coverage: coverage,
                deprecated_cheatcodes,
                inputs: &calls,
                generate_stack_trace,
                contract_decoder,
                fail_on_revert: failed_case.fail_on_revert,
                revert_decoder,
                retain_traces,
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
    if let Some(fuzzer) = &mut executor.inspector.fuzzer
        && let Some(call_generator) = &mut fuzzer.call_generator
    {
        call_generator.last_sequence = Arc::new(RwLock::new(inner_sequence.to_owned()));
        call_generator.set_replay(true);
    }
}
