//! Utility functions for blockchain implementations

use core::ops::Bound;
use std::collections::BTreeMap;

use edr_block_api::Block;
use edr_block_storage::ReservableSparseBlockStorage;
use edr_receipt::ReceiptTrait;
use edr_state_api::{StateCommit, StateDiff, StateOverride};

/// Computes the state at a given block by applying state diffs from local
/// blocks
pub fn compute_state_at_block<
    BlockReceiptT: Clone + ReceiptTrait,
    BlockT: Block<SignedTransactionT> + Clone,
    HardforkT: Clone,
    SignedTransactionT,
>(
    state: &mut dyn StateCommit,
    local_storage: &ReservableSparseBlockStorage<
        BlockReceiptT,
        BlockT,
        HardforkT,
        SignedTransactionT,
    >,
    first_local_block_number: u64,
    last_local_block_number: u64,
    state_overrides: &BTreeMap<u64, StateOverride>,
) {
    // If we're dealing with a local block, apply their state diffs
    let state_diffs = local_storage
        .state_diffs_until_block(last_local_block_number)
        .unwrap_or_default();

    let mut overriden_state_diffs: BTreeMap<u64, StateDiff> = state_diffs
        .iter()
        .map(|(block_number, state_diff)| (*block_number, state_diff.clone()))
        .collect();

    for (block_number, state_override) in state_overrides.range((
        Bound::Included(&first_local_block_number),
        Bound::Included(&last_local_block_number),
    )) {
        overriden_state_diffs
            .entry(*block_number)
            .and_modify(|state_diff| {
                state_diff.apply_diff(state_override.diff.as_inner().clone());
            })
            .or_insert_with(|| state_override.diff.clone());
    }

    for (_block_number, state_diff) in overriden_state_diffs {
        state.commit(state_diff.into());
    }
}
