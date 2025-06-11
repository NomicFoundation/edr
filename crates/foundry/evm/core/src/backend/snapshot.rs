use alloy_primitives::{
    map::{AddressHashMap, HashMap},
    B256, U256,
};
use revm::{context::JournalInner, state::AccountInfo, JournalEntry};
use serde::{Deserialize, Serialize};

use crate::evm_context::EvmEnv;

/// A minimal abstraction of a state at a certain point in time
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct StateSnapshot {
    pub accounts: AddressHashMap<AccountInfo>,
    pub storage: AddressHashMap<HashMap<U256, U256>>,
    pub block_hashes: HashMap<U256, B256>,
}

/// Represents a state snapshot taken during evm execution
#[derive(Clone, Debug)]
pub struct BackendStateSnapshot<DatabaseT, BlockT, TxT, HardforkT> {
    pub db: DatabaseT,
    /// The `journaled_state` state at a specific point
    pub journaled_state: JournalInner<JournalEntry>,
    /// Contains the env at the time of the snapshot
    pub env: EvmEnv<BlockT, TxT, HardforkT>,
}

// === impl BackendSnapshot ===

impl<DatabaseT, BlockT, TxT, HardforkT> BackendStateSnapshot<DatabaseT, BlockT, TxT, HardforkT> {
    /// Takes a new state snapshot.
    pub fn new(
        db: DatabaseT,
        journaled_state: JournalInner<JournalEntry>,
        env: EvmEnv<BlockT, TxT, HardforkT>,
    ) -> Self {
        Self {
            db,
            journaled_state,
            env,
        }
    }

    /// Called when this state snapshot is reverted.
    ///
    /// Since we want to keep all additional logs that were emitted since the
    /// snapshot was taken we'll merge additional logs into the snapshot's
    /// `revm::JournaledState`. Additional logs are those logs that are
    /// missing in the snapshot's `journaled_state`, since the current
    /// `journaled_state` includes the same logs, we can simply replace use that
    /// See also `DatabaseExt::revert`.
    pub fn merge(&mut self, current: &JournalInner<JournalEntry>) {
        self.journaled_state.logs.clone_from(&current.logs);
    }
}

/// What to do when reverting a state snapshot.
///
/// Whether to remove the state snapshot or keep it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum RevertStateSnapshotAction {
    /// Remove the state snapshot after reverting
    #[default]
    RevertRemove,
    /// Keep the state snapshot after reverting
    RevertKeep,
}

impl RevertStateSnapshotAction {
    /// Returns `true` if the action is to keep the state snapshot.
    pub fn is_keep(&self) -> bool {
        matches!(self, RevertStateSnapshotAction::RevertKeep)
    }
}
