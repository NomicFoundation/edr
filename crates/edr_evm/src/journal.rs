use edr_eth::log::ExecutionLog;
pub use revm::JournalEntry;
pub use revm_context::Journal;
pub use revm_context_interface::JournalTr as JournalTrait;

use crate::state::{Database, EvmState};

/// Extension trait for `Journal` to provide additional functionality.
pub trait JournalExt {
    type Entry;

    /// Retrieves the journal entries of state changes, one for each frame.
    fn entries(&self) -> &[Vec<Self::Entry>];

    /// Retrieves the emitted logs.
    fn logs(&self) -> &[ExecutionLog];

    /// Retrieves the current state.
    fn state(&self) -> &EvmState;
}

impl<DatabaseT: Database> JournalExt for Journal<DatabaseT> {
    type Entry = JournalEntry;

    fn entries(&self) -> &[Vec<Self::Entry>] {
        &self.journal
    }

    fn logs(&self) -> &[ExecutionLog] {
        &self.logs
    }

    fn state(&self) -> &EvmState {
        &self.state
    }
}
