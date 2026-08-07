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
use revm_inspectors::tracing::types::DecodedTraceStep;
pub use revm_inspectors::tracing::{
    types::{
        CallKind, CallLog, CallTrace, CallTraceNode, CallTraceStep, DecodedCallData,
        DecodedCallLog, DecodedCallTrace, TraceMemberOrder,
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
use identifier::LocalTraceIdentifier;

pub mod abi;
pub mod decoder;
pub use decoder::{CallTraceDecoder, CallTraceDecoderBuilder};
use foundry_evm_core::contracts::{ContractsByAddress, ContractsByArtifact};

/// A suite's setup-phase trace arenas, including deployments and `setUp()`,
/// in execution order — appended through
/// [`push_setup_trace_stripping_prior_steps`], so only the last one carries
/// recorded EVM steps. When setup failed, that last arena is the failing
/// call; the setup stack-trace computation relies on both properties.
pub type SetupTraces = Vec<SetupTrace>;

/// A setup trace arena paired with the kind of setup call that recorded it.
pub type SetupTrace = (SetupTraceKind, SparsedTraceArena);

/// Trace arena keeping track of ignored trace items.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SparsedTraceArena {
    /// Full trace arena.
    #[serde(flatten)]
    pub arena: CallTraceArena,
    /// Ranges of trace steps to ignore in format (`start_node`, `start_step`)
    /// -> (`end_node`, `end_step`).
    /// See `foundry_cheatcodes::utils::IgnoredTraces` for more information.
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub ignored: HashMap<(usize, usize), (usize, usize)>,
}

impl SparsedTraceArena {
    /// Goes over entire trace arena and removes ignored trace items.
    pub fn resolve_arena(&self) -> Cow<'_, CallTraceArena> {
        if self.ignored.is_empty() {
            Cow::Borrowed(&self.arena)
        } else {
            let mut arena = self.arena.clone();
            clear_node(arena.nodes_mut(), 0, &self.ignored, &mut None);
            Cow::Owned(arena)
        }
    }

    /// Removes the ignored trace items from the arena itself, so that
    /// [`resolve_arena`](Self::resolve_arena) no longer needs to clone it.
    /// The arena is no longer sparse afterwards; a no-op when it never was.
    pub fn resolve_in_place(&mut self) {
        if !self.ignored.is_empty() {
            clear_node(self.arena.nodes_mut(), 0, &self.ignored, &mut None);
            self.ignored = HashMap::default();
        }
    }

    /// Discards the recorded EVM steps, keeping the rest of the call tree:
    /// its nodes, their logs and their ordering. With step recording enabled
    /// the steps are by far the largest part of an arena — one entry per
    /// executed opcode — while in the Solidity test runner their only
    /// consumer is stack-trace generation.
    ///
    /// Must only be called once a stack trace can no longer be requested for
    /// this arena.
    ///
    /// Ignored ranges (from the `pauseTracing`/`resumeTracing` cheatcodes) are
    /// resolved first: they are keyed by position in each node's `ordering`,
    /// which dropping the step entries would shift. Afterwards the arena is
    /// no longer sparse and [`resolve_arena`](Self::resolve_arena) borrows it
    /// as is.
    pub fn strip_steps(&mut self) {
        self.resolve_in_place();
        strip_arena_steps(&mut self.arena);
    }
}

/// The arenas a test's execution has recorded so far, in execution order.
///
/// Only the last arena carries recorded EVM steps, because [`push`](Self::push)
/// — the only way to grow the collection — strips them from the arena it
/// displaces; that bounds the peak while a test is still running, not just
/// between tests. Only that last arena may therefore be named as the failing
/// trace of a stack-trace computation; every earlier arena may only serve as a
/// code source, which is walked for its CREATE nodes alone. The collection
/// derefs to a slice for reading; mutable access —
/// [`iter_mut`](Self::iter_mut), iterating `&mut`, and the step-stripping
/// methods — hands out the arenas one by one, so it cannot reorder them, and
/// callers use it only to decode and label them in place.
#[derive(Clone, Debug, Default)]
pub struct ExecutionTraces(Vec<SparsedTraceArena>);

impl ExecutionTraces {
    /// Appends `arena`, stripping the recorded EVM steps from the arena that
    /// was previously last (see [`SparsedTraceArena::strip_steps`]).
    ///
    /// The strip is unconditional: an arena is stripped when displaced even if
    /// the whole collection is freed once the test finishes — the in-test peak
    /// is worth the walk.
    pub fn push(&mut self, arena: SparsedTraceArena) {
        self.strip_last_steps();
        self.0.push(arena);
    }

    /// Returns an iterator over the arenas, for decoding and labelling them in
    /// place — mutations that cannot reintroduce recorded steps.
    pub fn iter_mut(&mut self) -> std::slice::IterMut<'_, SparsedTraceArena> {
        self.0.iter_mut()
    }

    /// Strips the recorded EVM steps from the last arena, if any — the strip
    /// [`push`](Self::push) would otherwise do when that arena is displaced.
    pub fn strip_last_steps(&mut self) {
        if let Some(last) = self.0.last_mut() {
            last.strip_steps();
        }
    }

    /// Strips the recorded EVM steps from every arena, including the last.
    ///
    /// Must only be called once a stack trace can no longer be requested for
    /// any of them.
    pub fn strip_steps(&mut self) {
        for arena in &mut self.0 {
            arena.strip_steps();
        }
    }
}

impl Deref for ExecutionTraces {
    type Target = [SparsedTraceArena];

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl IntoIterator for ExecutionTraces {
    type Item = SparsedTraceArena;
    type IntoIter = std::vec::IntoIter<SparsedTraceArena>;

    fn into_iter(self) -> Self::IntoIter {
        self.0.into_iter()
    }
}

impl<'a> IntoIterator for &'a mut ExecutionTraces {
    type Item = &'a mut SparsedTraceArena;
    type IntoIter = std::slice::IterMut<'a, SparsedTraceArena>;

    fn into_iter(self) -> Self::IntoIter {
        self.0.iter_mut()
    }
}

impl FromIterator<SparsedTraceArena> for ExecutionTraces {
    /// Collects through [`push`](Self::push), so every arena but the last is
    /// stripped of its steps.
    fn from_iter<I: IntoIterator<Item = SparsedTraceArena>>(iter: I) -> Self {
        let mut traces = Self::default();
        for arena in iter {
            traces.push(arena);
        }
        traces
    }
}

/// Appends a setup trace to `traces`, stripping the recorded EVM steps from
/// the arena that was previously last — the [`SetupTraces`] counterpart of
/// [`ExecutionTraces::push`], with the same contract: only the last setup
/// arena may be named as the failing trace.
pub fn push_setup_trace_stripping_prior_steps(
    traces: &mut SetupTraces,
    kind: SetupTraceKind,
    arena: SparsedTraceArena,
) {
    if let Some((_, previous)) = traces.last_mut() {
        previous.strip_steps();
    }
    traces.push((kind, arena));
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

/// Trace mode for execution traces.
#[derive(Clone, Copy, Debug, Default)]
pub enum TracingMode {
    /// Don't collect traces
    #[default]
    None,
    /// Collect traces without recording steps
    WithoutSteps,
    /// Collect traces with recorded steps
    WithSteps,
}

impl TracingMode {
    pub fn into_config(self) -> Option<TracingInspectorConfig> {
        let record_steps = match self {
            Self::None => return None,
            Self::WithoutSteps => false,
            Self::WithSteps => true,
        };

        Some(TracingInspectorConfig {
            record_steps,
            record_memory_snapshots: false,
            record_stack_snapshots: StackSnapshotType::None,
            record_state_diff: false,
            record_returndata_snapshots: false,
            record_opcodes_filter: None,
            exclude_precompile_calls: false,
            record_logs: true,
            record_immediate_bytes: false,
        })
    }
}

/// Removes the trace items covered by `ignored` from the sub-tree rooted at
/// `node_idx`, recursing into the children it visits. `cur_ignore_end` carries
/// the end of the range currently being skipped across that recursion, so a
/// range may start in one node and end in another.
fn clear_node(
    nodes: &mut [CallTraceNode],
    node_idx: usize,
    ignored: &HashMap<(usize, usize), (usize, usize)>,
    cur_ignore_end: &mut Option<(usize, usize)>,
) {
    // Take the ordering out for the duration rather than cloning it: with step
    // recording enabled it holds one entry per executed opcode. The loop reads
    // this node's `children` and `steps` through `nodes` but never its
    // `ordering`, and only recurses into children — distinct indices — so
    // nothing observes the gap.
    let mut ordering = std::mem::take(
        &mut nodes
            .get_mut(node_idx)
            .expect("node_idx should be within nodes bounds")
            .ordering,
    );
    // Prepend an additional None item to the ordering to handle the beginning of
    // the trace.
    let items = std::iter::once(None)
        .chain(ordering.iter().copied().map(Some))
        .enumerate();

    let mut internal_calls = Vec::new();
    let mut items_to_remove = BTreeSet::new();
    for (item_idx, item) in items {
        if let Some(end_node) = ignored.get(&(node_idx, item_idx)) {
            *cur_ignore_end = Some(*end_node);
        }

        let mut remove = cur_ignore_end.is_some() & item.is_some();

        match item {
            // we only remove calls if they did not start/pause tracing
            Some(TraceMemberOrder::Call(child_idx)) => {
                let node = nodes
                    .get(node_idx)
                    .expect("node_idx should be within nodes bounds");
                let &child_node_idx = node
                    .children
                    .get(child_idx)
                    .expect("child_idx should be within children bounds");
                clear_node(nodes, child_node_idx, ignored, cur_ignore_end);
                remove &= cur_ignore_end.is_some();
            }
            // we only remove decoded internal calls if they did not start/pause tracing
            Some(TraceMemberOrder::Step(step_idx)) => {
                // If this is an internal call beginning, track it in `internal_calls`
                let node = nodes
                    .get(node_idx)
                    .expect("node_idx should be within nodes bounds");
                let step = node
                    .trace
                    .steps
                    .get(step_idx)
                    .expect("step_idx should be within steps bounds");
                if let Some(decoded) = &step.decoded
                    && let DecodedTraceStep::InternalCall(_, end_step_idx) = &**decoded
                {
                    internal_calls.push((item_idx, remove, *end_step_idx));
                    // we decide if we should remove it later
                    remove = false;
                }
                // Handle ends of internal calls
                internal_calls.retain(|(start_item_idx, remove_start, end_idx)| {
                    if *end_idx != step_idx {
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

        if let Some((end_node, end_step_idx)) = cur_ignore_end
            && node_idx == *end_node
            && item_idx == *end_step_idx
        {
            *cur_ignore_end = None;
        }
    }

    for (offset, item_idx) in items_to_remove.into_iter().enumerate() {
        ordering.remove(item_idx - offset - 1);
    }
    nodes
        .get_mut(node_idx)
        .expect("node_idx should be within nodes bounds")
        .ordering = ordering;
}

/// Discards the recorded EVM steps of every node in `arena`, keeping the rest
/// of the call tree: its nodes, their logs and their ordering. See
/// [`SparsedTraceArena::strip_steps`] for when this is safe.
///
/// Must not be applied to the arena of a still-sparse [`SparsedTraceArena`]:
/// its ignored ranges are keyed by position in `ordering`, which this shifts.
/// Use [`SparsedTraceArena::strip_steps`] there instead.
pub fn strip_arena_steps(arena: &mut CallTraceArena) {
    for node in arena.nodes_mut() {
        node.trace.steps = Vec::new();
        node.ordering
            .retain(|item| !matches!(item, TraceMemberOrder::Step(_)));
        // `retain` keeps the capacity, which grew with one entry per step.
        node.ordering.shrink_to_fit();
    }
}

/// Decode a collection of call traces.
///
/// The traces will be decoded using the given decoder, if possible.
pub async fn decode_trace_arena(arena: &mut CallTraceArena, decoder: &CallTraceDecoder) {
    decoder.populate_traces(arena.nodes_mut()).await;
}

/// Render a collection of call traces to a string.
pub fn render_trace_arena(arena: &CallTraceArena) -> String {
    let mut w = TraceWriter::new(Vec::<u8>::new());
    w.write_arena(arena).expect("Failed to write traces");
    String::from_utf8(w.into_writer()).expect("trace writer wrote invalid UTF-8")
}

/// Specifies the kind of trace.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum SetupTraceKind {
    Deployment,
    Setup,
}

impl SetupTraceKind {
    /// Returns `true` if the trace kind is [`Deployment`].
    ///
    /// [`Deployment`]: SetupTraceKind::Deployment
    #[must_use]
    pub fn is_deployment(self) -> bool {
        matches!(self, Self::Deployment)
    }

    /// Returns `true` if the trace kind is [`Setup`].
    ///
    /// [`Setup`]: SetupTraceKind::Setup
    #[must_use]
    pub fn is_setup(self) -> bool {
        matches!(self, Self::Setup)
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
        for address in decoder.identify_addresses(trace, &mut local_identifier) {
            if let (Some(contract), Some(abi)) = (address.contract, address.abi) {
                contracts.insert(address.address, (contract, abi.into_owned()));
            }
        }
    }
    contracts
}

#[cfg(test)]
mod tests {
    use revm::bytecode::opcode::OpCode;

    use super::*;

    /// Returns a minimal recorded step; only its presence in a node matters.
    fn step() -> CallTraceStep {
        CallTraceStep {
            pc: 0,
            op: OpCode::STOP,
            stack: None,
            push_stack: None,
            memory: None,
            returndata: alloy_primitives::Bytes::default(),
            gas_remaining: 0,
            gas_refund_counter: 0,
            gas_used: 0,
            gas_cost: 0,
            storage_change: None,
            status: None,
            immediate_bytes: None,
            decoded: None,
        }
    }

    /// Root node with a `pauseTracing` child (node 1), a `resumeTracing` child
    /// (node 3) and a call in between (node 2) that should be ignored, with
    /// steps interleaved the way the tracing inspector records them.
    fn paused_arena() -> SparsedTraceArena {
        use TraceMemberOrder::{Call, Log, Step};

        let mut arena = CallTraceArena::default();
        let nodes = arena.nodes_mut();
        nodes[0].children = vec![1, 2, 3];
        nodes[0].logs = vec![CallLog::default(), CallLog::default()];
        nodes[0].trace.steps = (0..5).map(|_| step()).collect();
        nodes[0].ordering = vec![
            Step(0),
            Log(0),
            Step(1),
            Call(0),
            Step(2),
            Call(1),
            Log(1),
            Step(3),
            Call(2),
            Step(4),
        ];
        for idx in 1..=3 {
            nodes.push(CallTraceNode {
                parent: Some(0),
                idx,
                ..CallTraceNode::default()
            });
        }

        // Recorded by the cheatcodes as the position in the cheatcode call's
        // own (still empty) ordering.
        let mut ignored = HashMap::default();
        ignored.insert((1, 0), (3, 0));

        SparsedTraceArena { arena, ignored }
    }

    #[test]
    fn push_strips_steps_of_the_displaced_arena_only() {
        let mut traces = ExecutionTraces::default();

        traces.push(paused_arena());
        assert!(!traces[0].nodes()[0].trace.steps.is_empty());

        traces.push(paused_arena());
        assert!(traces[0].nodes()[0].trace.steps.is_empty());
        assert!(traces[0].ignored.is_empty());
        assert!(!traces[1].nodes()[0].trace.steps.is_empty());
    }

    #[test]
    fn strip_steps_matches_resolving_then_dropping_steps() {
        let mut arena = paused_arena();

        let expected: Vec<Vec<TraceMemberOrder>> = arena
            .resolve_arena()
            .nodes()
            .iter()
            .map(|node| {
                node.ordering
                    .iter()
                    .filter(|item| !matches!(item, TraceMemberOrder::Step(_)))
                    .copied()
                    .collect()
            })
            .collect();

        arena.strip_steps();

        assert!(arena.ignored.is_empty());
        assert!(matches!(arena.resolve_arena(), Cow::Borrowed(_)));
        for (node, expected) in arena.nodes().iter().zip(expected) {
            assert!(node.trace.steps.is_empty());
            assert_eq!(node.ordering, expected);
        }
    }
}
