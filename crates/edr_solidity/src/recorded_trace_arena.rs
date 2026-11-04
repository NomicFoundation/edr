//! Compact per-frame opcode steps, recorded next to a [`CallTraceArena`]
//! whose own `CallTraceStep`s are disabled.

use edr_primitives::{bytecode::opcode::OpCode, Bytes, U256};
use revm_inspectors::tracing::{
    types::{CallTrace, CallTraceNode},
    CallTraceArena,
};

/// An executed opcode.
///
/// Unlike `revm_inspectors`' `CallTraceStep`, it holds only what EDR reads from
/// steps: the PC and opcode, used by stack traces and `traces()`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct RecordedStep {
    /// Program counter before step execution
    pub pc: u32,
    /// Opcode to be executed
    pub op: OpCode,
}

/// The steps of a call trace node.
#[derive(Clone, Debug, Default)]
pub struct FrameSteps {
    /// The executed opcodes.
    pub(crate) steps: Vec<RecordedStep>,
    /// The stack before each step. Empty unless stack snapshots are recorded.
    pub(crate) stacks: Vec<Box<[U256]>>,
    /// The memory before each step. Empty unless memory snapshots are
    /// recorded.
    pub(crate) memories: Vec<Bytes>,
}

impl FrameSteps {
    /// Returns the number of executed opcodes.
    pub fn len(&self) -> usize {
        self.steps.len()
    }

    /// Returns whether no opcodes were executed.
    pub fn is_empty(&self) -> bool {
        self.steps.is_empty()
    }

    /// Returns each executed opcode with the snapshots taken before it.
    pub fn iter(&self) -> impl DoubleEndedIterator<Item = StepSnapshot<'_>> {
        self.steps
            .iter()
            .enumerate()
            .map(|(step_idx, step)| StepSnapshot {
                pc: step.pc,
                op: step.op,
                stack: self.stacks.get(step_idx).map(AsRef::as_ref),
                memory: self.memories.get(step_idx),
            })
    }
}

/// An executed opcode of a [`FrameSteps`], with its snapshots.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StepSnapshot<'a> {
    /// Program counter before step execution
    pub pc: u32,
    /// Opcode to be executed
    pub op: OpCode,
    /// The stack before the step, if recorded.
    pub stack: Option<&'a [U256]>,
    /// The memory before the step, if recorded.
    pub memory: Option<&'a Bytes>,
}

/// Records the steps of each node of a [`CallTraceArena`] while it is traced.
#[derive(Debug, Default)]
pub(crate) struct StepRecorder {
    /// Indexed like the arena's nodes; may omit trailing nodes that executed
    /// no opcodes.
    steps: Vec<FrameSteps>,
}

impl StepRecorder {
    /// Returns the last step recorded for the node at `node_idx`, if any.
    pub(crate) fn last_step(&self, node_idx: usize) -> Option<StepSnapshot<'_>> {
        self.steps.get(node_idx)?.iter().next_back()
    }

    /// Records `step` for the node at `node_idx`, with the stack and memory
    /// before it if they are snapshotted.
    pub(crate) fn record(
        &mut self,
        node_idx: usize,
        step: RecordedStep,
        stack: Option<Box<[U256]>>,
        memory: Option<Bytes>,
    ) {
        let node_steps = self.node_steps_mut(node_idx);
        if let Some(memory) = memory {
            node_steps.memories.push(memory);
        }
        if let Some(stack) = stack {
            node_steps.stacks.push(stack);
        }
        node_steps.steps.push(step);
    }

    /// Returns the steps of the node at `node_idx`.
    fn node_steps_mut(&mut self, node_idx: usize) -> &mut FrameSteps {
        self.pad_steps_to(node_idx + 1);
        self.steps
            .get_mut(node_idx)
            .expect("padded to include the node")
    }

    /// Ensures there are entries for the first `node_count` nodes, adding
    /// empty ones for the nodes that executed no opcodes. Never shrinks.
    ///
    /// Entry `i` holds the steps of the arena's node `i`. Creating entries
    /// lazily, when a step is recorded for a node or when finishing, keeps
    /// that alignment without tracking frames in the `call`/`create` hooks.
    fn pad_steps_to(&mut self, node_count: usize) {
        if self.steps.len() < node_count {
            self.steps.resize_with(node_count, FrameSteps::default);
        }
    }

    /// Pairs the recorded steps with `arena`, the arena they were recorded
    /// for.
    pub(crate) fn finish(mut self, arena: CallTraceArena) -> RecordedTraceArena {
        debug_assert!(
            self.steps.len() <= arena.nodes().len(),
            "steps recorded for nodes outside the arena"
        );
        self.pad_steps_to(arena.nodes().len());

        RecordedTraceArena {
            arena,
            steps: self.steps,
        }
    }
}

/// A call trace arena with the steps of each node.
#[derive(Clone, Debug)]
pub struct RecordedTraceArena {
    arena: CallTraceArena,
    /// One entry per node of `arena`; empty for nodes that executed no opcodes.
    steps: Vec<FrameSteps>,
}

impl RecordedTraceArena {
    /// Returns the call trace arena, without the recorded steps.
    pub fn as_call_trace_arena(&self) -> &CallTraceArena {
        &self.arena
    }

    /// Returns the node at `node_idx` with its steps, or `None` if the arena
    /// has no such node.
    pub fn node(&self, node_idx: usize) -> Option<RecordedNode<'_>> {
        Some(RecordedNode {
            arena: self,
            node: self.arena.nodes().get(node_idx)?,
            steps: self.steps.get(node_idx)?,
        })
    }

    /// Returns every node with its steps, in arena order.
    pub fn nodes(&self) -> impl ExactSizeIterator<Item = RecordedNode<'_>> {
        self.arena
            .nodes()
            .iter()
            .zip(&self.steps)
            .map(|(node, steps)| RecordedNode {
                arena: self,
                node,
                steps,
            })
    }
}

impl Default for RecordedTraceArena {
    fn default() -> Self {
        StepRecorder::default().finish(CallTraceArena::default())
    }
}

/// A node of a [`RecordedTraceArena`] with its steps.
#[derive(Clone, Copy, Debug)]
pub struct RecordedNode<'arena> {
    arena: &'arena RecordedTraceArena,
    node: &'arena CallTraceNode,
    steps: &'arena FrameSteps,
}

impl<'arena> RecordedNode<'arena> {
    /// Returns the node's call trace.
    pub fn trace(&self) -> &'arena CallTrace {
        &self.node.trace
    }

    /// Returns the node's steps.
    pub fn steps(&self) -> &'arena FrameSteps {
        self.steps
    }

    /// Returns the node's child calls, in execution order.
    pub fn children(&self) -> impl Iterator<Item = RecordedNode<'arena>> + 'arena {
        let arena = self.arena;
        self.node
            .children
            .iter()
            .map(move |&node_idx| arena.node(node_idx).expect("children belong to the arena"))
    }
}

/// A call trace arena, with where the steps of its nodes are read from.
#[derive(Clone, Copy, Debug)]
pub enum ArenaWithSteps<'arena> {
    /// Steps recorded by the arena's own `TracingInspector`.
    Traced(&'arena CallTraceArena),
    /// Steps recorded next to the arena.
    Recorded(&'arena RecordedTraceArena),
}

impl<'arena> ArenaWithSteps<'arena> {
    /// Returns the call trace arena.
    pub(crate) fn as_call_trace_arena(self) -> &'arena CallTraceArena {
        match self {
            ArenaWithSteps::Traced(arena) => arena,
            ArenaWithSteps::Recorded(recorded) => recorded.as_call_trace_arena(),
        }
    }

    /// Returns the steps of `node`, a node of the arena.
    pub(crate) fn node_steps(
        self,
        node: &'arena CallTraceNode,
    ) -> Box<dyn Iterator<Item = RecordedStep> + 'arena> {
        match self {
            ArenaWithSteps::Traced(_) => {
                Box::new(node.trace.steps.iter().map(|step| RecordedStep {
                    pc: step.pc as u32,
                    op: step.op,
                }))
            }
            ArenaWithSteps::Recorded(recorded) => Box::new(
                recorded
                    .node(node.idx)
                    .expect("node belongs to the arena")
                    .steps()
                    .steps
                    .iter()
                    .copied(),
            ),
        }
    }
}

impl<'arena> From<&'arena CallTraceArena> for ArenaWithSteps<'arena> {
    fn from(arena: &'arena CallTraceArena) -> Self {
        ArenaWithSteps::Traced(arena)
    }
}

impl<'arena> From<&'arena RecordedTraceArena> for ArenaWithSteps<'arena> {
    fn from(arena: &'arena RecordedTraceArena) -> Self {
        ArenaWithSteps::Recorded(arena)
    }
}
