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
    backend::IndeterminismReasons,
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
use foundry_evm_traces::{load_contracts, ExecutionTraces, SetupTraces, TracingMode};
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
    Executor, RawCallResult,
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
    pub setup_traces: &'a SetupTraces,
    pub line_coverage: &'a mut Option<HitMaps>,
    pub deprecated_cheatcodes: &'a mut HashMap<&'static str, Option<&'static str>>,
    pub inputs: &'a [BasicTxDetails],
    pub generate_stack_trace: bool,
    /// Must be provided if `generate_stack_trace` is true
    pub contract_decoder: Option<&'a NestedTraceDecoderT>,
    pub revert_decoder: &'a RevertDecoder,
    pub fail_on_revert: bool,
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
    /// What a reverting call returned. The revert reason is decoded from
    /// these fields.
    struct Failure {
        output: Bytes,
        exit_reason: Option<InstructionResult>,
    }

    let ReplayRunArgs {
        execution_traces,
        mut executor,
        invariant_contract,
        known_contracts,
        mut ided_contracts,
        logs,
        setup_traces,
        line_coverage: coverage,
        deprecated_cheatcodes,
        inputs,
        generate_stack_trace,
        contract_decoder,
        revert_decoder,
        fail_on_revert,
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
        let RawCallResult {
            exit_reason,
            reverted,
            has_state_snapshot_failure: _,
            result,
            gas_used: _,
            gas_refunded: _,
            stipend: _,
            logs: call_logs,
            labels: _,
            call_trace_arena,
            line_coverage,
            edge_coverage: _,
            state_changeset: _,
            env: _,
            cheatcodes: _,
            out: _,
            reverter: _,
            indeterminism_reasons,
        } = executor.transact_raw(
            tx.sender,
            tx.call_details.target,
            tx.call_details.calldata.clone(),
            U256::ZERO,
        )?;
        logs.extend(call_logs);
        HitMaps::merge_opt(coverage, line_coverage);

        // Identify newly generated contracts, if they exist.
        ided_contracts.extend(load_contracts(
            call_trace_arena.iter().map(|a| &a.arena),
            known_contracts,
        ));

        execution_traces.push(call_trace_arena.expect("enabled tracing"));

        // Create counter example to be used in failed case.
        counterexample_sequence.push(BaseCounterExample::from_invariant_call(
            tx.sender,
            tx.call_details.target,
            &tx.call_details.calldata,
            &ided_contracts,
            // Counterexample arenas are never consumed; the failing arena
            // lives on in `execution_traces`.
            None,
            /* indeterminism_reason */ None,
        ));

        // If this call failed, but didn't revert, this is terminal for sure.
        // If this call reverted, only exit if `fail_on_revert` is true.
        if !exit_reason.is_some_and(InstructionResult::is_ok) && (fail_on_revert || !reverted) {
            let stack_trace_result = if let Some(indeterminism_reasons) = indeterminism_reasons {
                Some(indeterminism_reasons.into())
            } else {
                contract_decoder.map(|decoder| {
                    let (failing_trace, prior_traces) = execution_traces
                        .split_last()
                        .expect("an arena was pushed for this call above");

                    get_stack_trace(
                        decoder,
                        &failing_trace.arena,
                        setup_traces
                            .iter()
                            .map(|(_, arena)| &arena.arena)
                            .chain(prior_traces.iter().map(|arena| &arena.arena)),
                        DeployedCode::default(),
                    )
                    .map_err(SolidityTestStackTraceError::from)
                    .into()
                })
            };
            let revert_reason = revert_decoder.maybe_decode(result.as_ref(), exit_reason);
            return Ok(ReplayResult {
                counterexample_sequence,
                stack_trace_result,
                revert_reason,
            });
        }

        // This call is not the failing one, so its arena can only ever serve
        // as a code source. Strip it now rather than when the next push
        // displaces it. Then no arena in the collection is step-laden while
        // the next call runs.
        execution_traces.strip_last_steps();
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

    execution_traces.push(invariant_result.call_trace_arena.expect("tracing is on"));
    logs.extend(invariant_result.logs);
    deprecated_cheatcodes.extend(
        invariant_result
            .cheatcodes
            .as_ref()
            .map_or_else(Default::default, |cheats| cheats.deprecated.clone()),
    );

    // Collect after invariant logs and traces. When `afterInvariant()` is
    // what failed, the revert reason must be decoded from it rather than from
    // the passing `invariant()` call.
    let mut after_invariant_failure: Option<Failure> = None;
    let mut after_invariant_indeterminism: Option<IndeterminismReasons> = None;
    if invariant_contract.call_after_invariant && invariant_success {
        let CallAfterInvariantResult {
            call_result: after_invariant_result,
            success: after_invariant_success,
        } = call_after_invariant_function(&executor, invariant_contract.address)?;
        execution_traces.push(
            after_invariant_result
                .call_trace_arena
                .expect("tracing is on"),
        );
        after_invariant_indeterminism = after_invariant_result.indeterminism_reasons;
        if !after_invariant_success {
            after_invariant_failure = Some(Failure {
                output: after_invariant_result.result,
                exit_reason: after_invariant_result.exit_reason,
            });
        }
        logs.extend(after_invariant_result.logs);
    }

    // Replay safety covers every call the replay executed, whichever one
    // failed. The sequence's persisted impurity is included in both tail
    // results. `invariant()` and `afterInvariant()` run on copy-on-write
    // backends, so each result only carries its own impurity and the two
    // must be merged. A failure that did not reproduce can also be explained
    // by impurity observed anywhere in the replay.
    let indeterminism_reasons = match (
        invariant_result.indeterminism_reasons,
        after_invariant_indeterminism,
    ) {
        (Some(mut reasons), other) => {
            reasons.merge(other);
            Some(reasons)
        }
        (None, other) => other,
    };

    let Failure {
        output: failing_output,
        exit_reason: failing_exit_reason,
    } = after_invariant_failure.unwrap_or_else(|| Failure {
        output: invariant_result.result,
        exit_reason: invariant_result.exit_reason,
    });

    let stack_trace_result: Option<SolidityTestStackTraceResult<HaltReasonT>> =
        generate_stack_trace
            .then(|| {
                indeterminism_reasons
                    .map(SolidityTestStackTraceResult::from)
                    .or_else(|| {
                        contract_decoder.map(|decoder| {
                            let (failing_trace, prior_traces) = execution_traces.split_last().expect(
                                "the failing call's arena was pushed above: afterInvariant() when it ran, otherwise invariant()",
                            );

                            get_stack_trace(
                                decoder,
                                &failing_trace.arena,
                                setup_traces
                                    .iter()
                                    .map(|(_, arena)| &arena.arena)
                                    .chain(prior_traces.iter().map(|arena| &arena.arena)),
                                DeployedCode::default(),
                            )
                            .map_err(SolidityTestStackTraceError::from)
                            .into()
                        })
                    })
            })
            .flatten();

    let revert_reason = revert_decoder.maybe_decode(failing_output.as_ref(), failing_exit_reason);

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
    pub setup_traces: &'a SetupTraces,
    pub coverage: &'a mut Option<HitMaps>,
    pub deprecated_cheatcodes: &'a mut HashMap<&'static str, Option<&'static str>>,
    pub generate_stack_trace: bool,
    /// Must be provided if `generate_stack_trace` is true
    pub contract_decoder: Option<&'a NestedTraceDecoderT>,
    pub revert_decoder: &'a RevertDecoder,
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
        setup_traces,
        coverage,
        deprecated_cheatcodes,
        generate_stack_trace,
        contract_decoder,
        revert_decoder,
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
                setup_traces,
                line_coverage: coverage,
                deprecated_cheatcodes,
                inputs: &calls,
                generate_stack_trace,
                contract_decoder,
                fail_on_revert: failed_case.fail_on_revert,
                revert_decoder,
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
