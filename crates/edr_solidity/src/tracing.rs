//! Types and utilities for tracing EVM execution with Solidity-specific
//! decoding.

use std::sync::Arc;

use edr_primitives::{Address, Bytes, HashMap, HashSet};
use parking_lot::RwLock;
use revm_inspectors::tracing::{CallTraceArena, TracingInspector};

use crate::contract_decoder::ContractDecoder;

/// A tracing inspector that uses a [`ContractDecoder`] to decode
/// Solidity-specific information.
///
/// The [`TracingInspector`] does not store the bytecode of executed bytecode
/// for call transactions, so we need to store them here to be able to decode
/// the traces properly.
pub struct SolidityTracingInspector {
    address_to_runtime_code: HashMap<Address, Bytes>,
    decoder: Arc<RwLock<ContractDecoder>>,
    inspector: TracingInspector,
}

impl SolidityTracingInspector {
    /// Constructs a new [`SolidityTracingInspector`] instance.
    pub fn new(inspector: TracingInspector, decoder: Arc<ContractDecoder>) -> Self {
        Self {
            decoder,
            address_to_runtime_code: HashMap::default(),
            inspector,
        }
    }

    pub fn collect(self, precompiles: HashSet<Address>) -> CallTraceArena {
        let mut arena = self.inspector.into_traces();

        for node in arena.nodes_mut() {
            self.decoder
                .populate_call_trace(&mut node.trace, code, precompile_spec_id);
        }

        arena
    }
}
