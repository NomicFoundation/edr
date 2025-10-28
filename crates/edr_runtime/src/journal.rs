use edr_evm_spec::{Database, Journal, JournalEntry};
use edr_receipt::log::ExecutionLog;
use edr_state_api::EvmState;

/// Extension trait for `Journal` to provide additional functionality.
pub trait JournalExt {
    /// The type of journal entry.
    type Entry;

    /// Retrieves the journal entries of state changes, one for each frame.
    fn entries(&self) -> &[Self::Entry];

    /// Retrieves the emitted logs.
    fn logs(&self) -> &[ExecutionLog];

    /// Retrieves the current state.
    fn state(&self) -> &EvmState;
}

impl<DatabaseT: Database> JournalExt for Journal<DatabaseT> {
    type Entry = JournalEntry;

    fn entries(&self) -> &[Self::Entry] {
        &self.journal
    }

    fn logs(&self) -> &[ExecutionLog] {
        &self.logs
    }

    fn state(&self) -> &EvmState {
        &self.state
    }
}
