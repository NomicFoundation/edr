//! # foundry-evm-traces
//!
//! EVM trace identifying and decoding.

#![cfg_attr(not(test), warn(unused_crate_dependencies))]
#![cfg_attr(docsrs, feature(doc_cfg, doc_auto_cfg))]

#[macro_use]
extern crate tracing;

use std::{
    borrow::Cow,
    collections::BTreeSet,
    ops::{Deref, DerefMut},
};

use alloy_primitives::map::HashMap;
use revm_inspectors::tracing::types::{DecodedTraceStep, TraceMemberOrder};
pub use revm_inspectors::tracing::{
    types::{
        CallKind, CallLog, CallTrace, CallTraceNode, CallTraceStep, DecodedCallData,
        DecodedCallLog, DecodedCallTrace,
    },
    CallTraceArena, FourByteInspector, GethTraceBuilder, ParityTraceBuilder, StackSnapshotType,
    TraceWriter, TracingInspector, TracingInspectorConfig,
};
use serde::{Deserialize, Serialize};

/// Call trace address identifiers.
///
/// Identifiers figure out what ABIs and labels belong to all the addresses of
/// the trace.
pub mod identifier;
use identifier::{LocalTraceIdentifier, TraceIdentifier};

pub mod abi;
mod decoder;

pub use decoder::{CallTraceDecoder, CallTraceDecoderBuilder};
use foundry_evm_core::contracts::{ContractsByAddress, ContractsByArtifact};

pub type Traces = Vec<(TraceKind, SparsedTraceArena)>;

/// Trace arena keeping track of ignored trace items.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SparsedTraceArena {
    /// Full trace arena.
    #[serde(flatten)]
    pub arena: CallTraceArena,
    /// Ranges of trace steps to ignore in format (`start_node`, `start_step`)
    /// -> (`end_node`, `end_step`). See
    /// `foundry_cheatcodes::utils::IgnoredTraces` for more information.
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub ignored: HashMap<(usize, usize), (usize, usize)>,
}

impl SparsedTraceArena {
    /// Goes over entire trace arena and removes ignored trace items.
    #[allow(dead_code)]
    fn resolve_arena(&self) -> Cow<'_, CallTraceArena> {
        if self.ignored.is_empty() {
            Cow::Borrowed(&self.arena)
        } else {
            let mut arena = self.arena.clone();
            clear_node(arena.nodes_mut(), 0, &self.ignored, &mut None);
            Cow::Owned(arena)
        }
    }
}

fn clear_node(
    nodes: &mut [CallTraceNode],
    node_idx: usize,
    ignored: &HashMap<(usize, usize), (usize, usize)>,
    cur_ignore_end: &mut Option<(usize, usize)>,
) {
    // Prepend an additional None item to the ordering to handle the beginning of
    // the trace.
    let items = std::iter::once(None)
        .chain(nodes[node_idx].ordering.clone().into_iter().map(Some))
        .enumerate();

    let mut iternal_calls = Vec::new();
    let mut items_to_remove = BTreeSet::new();
    for (item_idx, item) in items {
        if let Some(end_node) = ignored.get(&(node_idx, item_idx)) {
            *cur_ignore_end = Some(*end_node);
        }

        let mut remove = cur_ignore_end.is_some() & item.is_some();

        match item {
            // we only remove calls if they did not start/pause tracing
            Some(TraceMemberOrder::Call(child_idx)) => {
                clear_node(
                    nodes,
                    nodes[node_idx].children[child_idx],
                    ignored,
                    cur_ignore_end,
                );
                remove &= cur_ignore_end.is_some();
            }
            // we only remove decoded internal calls if they did not start/pause tracing
            Some(TraceMemberOrder::Step(step_idx)) => {
                // If this is an internal call beginning, track it in `iternal_calls`
                if let Some(DecodedTraceStep::InternalCall(_, end_step_idx)) =
                    &nodes[node_idx].trace.steps[step_idx].decoded
                {
                    iternal_calls.push((item_idx, remove, *end_step_idx));
                    // we decide if we should remove it later
                    remove = false;
                }
                // Handle ends of internal calls
                iternal_calls.retain(|(start_item_idx, remove_start, end_step_idx)| {
                    if *end_step_idx != step_idx {
                        return true;
                    }
                    // only remove start if end should be removed as well
                    if *remove_start && remove {
                        items_to_remove.insert(*start_item_idx);
                    } else {
                        remove = false;
                    }

                    false
                });
            }
            _ => {}
        }

        if remove {
            items_to_remove.insert(item_idx);
        }

        if let Some((end_node, end_step_idx)) = cur_ignore_end {
            if node_idx == *end_node && item_idx == *end_step_idx {
                *cur_ignore_end = None;
            }
        }
    }

    for (offset, item_idx) in items_to_remove.into_iter().enumerate() {
        nodes[node_idx].ordering.remove(item_idx - offset - 1);
    }
}

impl Deref for SparsedTraceArena {
    type Target = CallTraceArena;

    fn deref(&self) -> &Self::Target {
        &self.arena
    }
}

impl DerefMut for SparsedTraceArena {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.arena
    }
}

/// Decode a collection of call traces.
///
/// The traces will be decoded using the given decoder, if possible.
pub async fn decode_trace_arena(
    arena: &mut CallTraceArena,
    decoder: &CallTraceDecoder,
) -> Result<(), std::fmt::Error> {
    decoder.prefetch_signatures(arena.nodes()).await;
    decoder.populate_traces(arena.nodes_mut()).await;

    Ok(())
}

/// Render a collection of call traces to a string.
pub fn render_trace_arena(arena: &CallTraceArena) -> String {
    let mut w = TraceWriter::new(Vec::<u8>::new());
    w.write_arena(arena).expect("Failed to write traces");
    String::from_utf8(w.into_writer()).expect("trace writer wrote invalid UTF-8")
}

/// Specifies the kind of trace.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum TraceKind {
    Deployment,
    Setup,
    Execution,
}

impl TraceKind {
    /// Returns `true` if the trace kind is [`Deployment`].
    ///
    /// [`Deployment`]: TraceKind::Deployment
    #[must_use]
    pub fn is_deployment(self) -> bool {
        matches!(self, Self::Deployment)
    }

    /// Returns `true` if the trace kind is [`Setup`].
    ///
    /// [`Setup`]: TraceKind::Setup
    #[must_use]
    pub fn is_setup(self) -> bool {
        matches!(self, Self::Setup)
    }

    /// Returns `true` if the trace kind is [`Execution`].
    ///
    /// [`Execution`]: TraceKind::Execution
    #[must_use]
    pub fn is_execution(self) -> bool {
        matches!(self, Self::Execution)
    }
}

/// Given a list of traces and artifacts, it returns a map connecting address to
/// abi
pub fn load_contracts<'a>(
    traces: impl IntoIterator<Item = &'a CallTraceArena>,
    known_contracts: &ContractsByArtifact,
) -> ContractsByAddress {
    let mut local_identifier = LocalTraceIdentifier::new(known_contracts);
    let decoder = CallTraceDecoder::new();
    let mut contracts = ContractsByAddress::new();
    for trace in traces {
        for address in local_identifier.identify_addresses(decoder.trace_addresses(trace)) {
            if let (Some(contract), Some(abi)) = (address.contract, address.abi) {
                contracts.insert(address.address, (contract, abi.into_owned()));
            }
        }
    }
    contracts
}
