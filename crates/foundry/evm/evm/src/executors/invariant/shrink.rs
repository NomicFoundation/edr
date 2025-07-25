// Removed unused import

use alloy_primitives::{Address, Bytes, U256};
use foundry_evm_core::{
    constants::MAGIC_ASSUME,
    evm_context::{
        BlockEnvTr, ChainContextTr, EvmBuilderTrait, HardforkTr, TransactionEnvTr,
        TransactionErrorTrait,
    },
};
use foundry_evm_fuzz::invariant::BasicTxDetails;
use proptest::bits::{BitSetLike, VarBitSet};
use revm::context::result::{HaltReason, HaltReasonTr};

use crate::executors::{
    invariant::{
        call_after_invariant_function, call_invariant_function, error::FailedInvariantCaseData,
        CallAfterInvariantResult, CallInvariantResult,
    },
    Executor,
};

#[derive(Clone, Copy, Debug)]
struct Shrink {
    call_index: usize,
}

/// Shrinker for a call sequence failure.
/// Iterates sequence call sequence top down and removes calls one by one.
/// If the failure is still reproducible with removed call then moves to the
/// next one. If the failure is not reproducible then restore removed call and
/// moves to next one.
#[derive(Debug)]
struct CallSequenceShrinker {
    /// Length of call sequence to be shrunk.
    call_sequence_len: usize,
    /// Call ids contained in current shrunk sequence.
    included_calls: VarBitSet,
    /// Current shrunk call id.
    shrink: Shrink,
    /// Previous shrunk call id.
    prev_shrink: Option<Shrink>,
}

impl CallSequenceShrinker {
    fn new(call_sequence_len: usize) -> Self {
        Self {
            call_sequence_len,
            included_calls: VarBitSet::saturated(call_sequence_len),
            shrink: Shrink { call_index: 0 },
            prev_shrink: None,
        }
    }

    /// Return candidate shrink sequence to be tested, by removing ids from
    /// original sequence.
    fn current(&self) -> impl Iterator<Item = usize> + '_ {
        (0..self.call_sequence_len).filter(|&call_id| self.included_calls.test(call_id))
    }

    /// Removes next call from sequence.
    fn simplify(&mut self) -> bool {
        if self.shrink.call_index >= self.call_sequence_len {
            // We reached the end of call sequence, nothing left to simplify.
            false
        } else {
            // Remove current call.
            self.included_calls.clear(self.shrink.call_index);
            // Record current call as previous call.
            self.prev_shrink = Some(self.shrink);
            // Remove next call index
            self.shrink = Shrink {
                call_index: self.shrink.call_index + 1,
            };
            true
        }
    }

    /// Reverts removed call from sequence and tries to simplify next call.
    fn complicate(&mut self) -> bool {
        match self.prev_shrink {
            Some(shrink) => {
                // Undo the last call removed.
                self.included_calls.set(shrink.call_index);
                self.prev_shrink = None;
                // Try to simplify next call.
                self.simplify()
            }
            None => false,
        }
    }
}

/// Shrinks the failure case to its smallest sequence of calls.
///
/// Maximal shrinkage is guaranteed if the `shrink_run_limit` is not set to a
/// value lower than the length of failed call sequence.
///
/// The shrunk call sequence always respect the order failure is reproduced as
/// it is tested top-down.
pub(crate) fn shrink_sequence<
    BlockT: BlockEnvTr,
    TxT: TransactionEnvTr,
    EvmBuilderT: 'static
        + EvmBuilderTrait<BlockT, ChainContextT, HaltReasonT, HardforkT, TransactionErrorT, TxT>,
    HaltReasonT: 'static + HaltReasonTr + TryInto<HaltReason>,
    HardforkT: HardforkTr,
    TransactionErrorT: TransactionErrorTrait,
    ChainContextT: 'static + ChainContextTr,
>(
    failed_case: &FailedInvariantCaseData,
    calls: &[BasicTxDetails],
    executor: &Executor<
        BlockT,
        TxT,
        EvmBuilderT,
        HaltReasonT,
        HardforkT,
        TransactionErrorT,
        ChainContextT,
    >,
    call_after_invariant: bool,
) -> eyre::Result<Vec<BasicTxDetails>> {
    trace!(target: "forge::test", "Shrinking sequence of {} calls.", calls.len());

    // Special case test: the invariant is *unsatisfiable* - it took 0 calls to
    // break the invariant -- consider emitting a warning.
    let CallInvariantResult {
        call_result: _,
        success,
        cow_backend: _,
    } = call_invariant_function(executor, failed_case.addr, failed_case.calldata.clone())?;
    if !success {
        return Ok(vec![]);
    }

    let mut shrinker = CallSequenceShrinker::new(calls.len());
    for _ in 0..failed_case.shrink_run_limit {
        // Check candidate sequence result.
        match check_sequence(
            executor.clone(),
            calls,
            shrinker.current().collect(),
            failed_case.addr,
            failed_case.calldata.clone(),
            failed_case.fail_on_revert,
            call_after_invariant,
        ) {
            // If candidate sequence still fails then shrink more if possible.
            Ok((false, _)) if !shrinker.simplify() => break,
            // If candidate sequence pass then restore last removed call and shrink other
            // calls if possible.
            Ok((true, _)) if !shrinker.complicate() => break,
            _ => {}
        }
    }

    Ok(shrinker.current().map(|idx| &calls[idx]).cloned().collect())
}

/// Checks if the given call sequence breaks the invariant.
///
/// Used in shrinking phase for checking candidate sequences and in replay
/// failures phase to test persisted failures.
/// Returns the result of invariant check (and afterInvariant call if needed)
/// and if sequence was entirely applied.
pub fn check_sequence<
    BlockT: BlockEnvTr,
    TxT: TransactionEnvTr,
    EvmBuilderT: 'static
        + EvmBuilderTrait<BlockT, ChainContextT, HaltReasonT, HardforkT, TransactionErrorT, TxT>,
    HaltReasonT: 'static + HaltReasonTr + TryInto<HaltReason>,
    HardforkT: HardforkTr,
    TransactionErrorT: TransactionErrorTrait,
    ChainContextT: 'static + ChainContextTr,
>(
    mut executor: Executor<
        BlockT,
        TxT,
        EvmBuilderT,
        HaltReasonT,
        HardforkT,
        TransactionErrorT,
        ChainContextT,
    >,
    calls: &[BasicTxDetails],
    sequence: Vec<usize>,
    test_address: Address,
    calldata: Bytes,
    fail_on_revert: bool,
    call_after_invariant: bool,
) -> eyre::Result<(bool, bool)> {
    // Apply the call sequence.
    for call_index in sequence {
        let tx = &calls[call_index];
        let call_result = executor.call_raw_committing(
            tx.sender,
            tx.call_details.target,
            tx.call_details.calldata.clone(),
            U256::ZERO,
        )?;
        // Ignore calls reverted with `MAGIC_ASSUME`. This is needed to handle failed
        // scenarios that are replayed with a modified version of test driver
        // (that use new `vm.assume` cheatcodes).
        if call_result.reverted && fail_on_revert && call_result.result.as_ref() != MAGIC_ASSUME {
            // Candidate sequence fails test.
            // We don't have to apply remaining calls to check sequence.
            return Ok((false, false));
        }
    }

    // Check the invariant for call sequence.
    let CallInvariantResult {
        call_result: _,
        mut success,
        cow_backend: _,
    } = call_invariant_function(&executor, test_address, calldata)?;
    // Check after invariant result if invariant is success and `afterInvariant`
    // function is declared.
    if success && call_after_invariant {
        CallAfterInvariantResult {
            call_result: _,
            success,
        } = call_after_invariant_function(&executor, test_address)?;
    }

    Ok((success, true))
}
