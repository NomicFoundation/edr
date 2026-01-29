//! A reusable inspector for collecting executed bytecode during EVM execution.
//!
//! This crate provides [`ExecutedBytecodeCollector`], which records the mapping
//! from contract addresses to their executed bytecode. This is useful for
//! decoding call traces and other debugging purposes.

use edr_chain_spec_evm::{
    interpreter::{CallInputs, CallOutcome, Gas, InstructionResult, InterpreterResult},
    ContextError, ContextTrait, Database as _, Inspector, JournalTrait,
};
use edr_primitives::{Address, Bytes, HashMap};
use revm_inspector::JournalExt;

/// An inspector that collects bytecode executed during EVM calls.
///
/// The [`revm_inspectors::tracing::TracingInspector`] does not store the
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
    fn call(&mut self, context: &mut ContextT, inputs: &mut CallInputs) -> Option<CallOutcome> {
        let code = inputs
            .known_bytecode
            .as_ref()
            .map_or_else(
                || {
                    // We need to use `map` before `unwrap_or_else` to please the borrow checker.
                    #[allow(clippy::map_unwrap_or)]
                    context
                        .journal()
                        .evm_state()
                        .get(&inputs.bytecode_address)
                        // Clone to end the borrow of the journal
                        .map(|account| Ok(account.info.clone()))
                        .unwrap_or_else(|| {
                            context
                                .journal_mut()
                                .db_mut()
                                .basic(inputs.bytecode_address)
                                // If an invalid contract address was provided, return empty code
                                .map(Option::unwrap_or_default)
                        })
                        .and_then(|account_info| {
                            account_info.code.map_or_else(
                                || {
                                    context
                                        .journal_mut()
                                        .db_mut()
                                        .code_by_hash(account_info.code_hash)
                                },
                                Ok,
                            )
                        })
                },
                |(_, bytecode)| Ok(bytecode.clone()),
            )
            // Get the original bytes for proper decoding
            .map(|code| code.original_bytes());

        let code = match code {
            Ok(code) => code,
            Err(error) => {
                *context.error() = Err(ContextError::Db(error));
                return Some(CallOutcome::new(
                    InterpreterResult::new(
                        InstructionResult::FatalExternalError,
                        Bytes::new(),
                        Gas::new(0),
                    ),
                    inputs.return_memory_offset.clone(),
                ));
            }
        };

        self.address_to_executed_code
            .insert(inputs.bytecode_address, code);

        None
    }
}
