//! A reusable inspector for collecting executed bytecode during EVM execution.
//!
//! This crate provides [`ExecutedBytecodeCollector`], which records the mapping
//! from contract addresses to their executed bytecode. This is useful for
//! decoding call traces and other debugging purposes.

use edr_chain_spec_evm::{
    interpreter::{CallInputs, CallOutcome},
    ContextTrait, Inspector,
};
use edr_primitives::{Address, Bytes, HashMap};
use revm_inspector::JournalExt;

/// An inspector that collects bytecode executed during EVM calls.
///
/// The `revm_inspectors::tracing::TracingInspector` does not store the
/// bytecode of executed code for call transactions, so we need to store them
/// separately to be able to decode the traces properly.
#[derive(Clone, Debug, Default)]
pub struct ExecutedBytecodeCollector {
    address_to_executed_code: HashMap<Address, Bytes>,
}

impl ExecutedBytecodeCollector {
    /// Consumes the collector and returns the collected bytecode mapping.
    pub fn collect(self) -> HashMap<Address, Bytes> {
        self.address_to_executed_code
    }

    /// Replaces the current collected bytecode with an empty map, returning
    /// the previous mapping.
    pub fn take(&mut self) -> HashMap<Address, Bytes> {
        std::mem::take(&mut self.address_to_executed_code)
    }
}

impl<ContextT: ContextTrait<Journal: JournalExt>> Inspector<ContextT>
    for ExecutedBytecodeCollector
{
    fn call(&mut self, _context: &mut ContextT, inputs: &mut CallInputs) -> Option<CallOutcome> {
        // Since `revm` 35, `CallInputs::known_bytecode` is always populated by the
        // handler with the resolved code/hash for the call target (including
        // EIP-7702 delegation lookups), so we no longer need to fall back to
        // fetching from the journal/database here.
        let code = inputs.known_bytecode.1.original_bytes();

        self.address_to_executed_code
            .insert(inputs.bytecode_address, code);

        None
    }
}
