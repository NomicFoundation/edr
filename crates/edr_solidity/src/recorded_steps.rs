//! Compact per-frame opcode steps, recorded next to a [`CallTraceArena`]
//! whose own `CallTraceStep`s are disabled.

use std::ops::Deref;

use edr_primitives::{bytecode::opcode::OpCode, Bytes, U256};
use revm_inspectors::tracing::CallTraceArena;

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
    pub fn iter(&self) -> impl Iterator<Item = FrameStep<'_>> {
        self.steps
            .iter()
            .enumerate()
            .map(|(step_idx, step)| FrameStep {
                pc: step.pc,
                op: step.op,
                stack: self.stacks.get(step_idx).map(AsRef::as_ref),
                memory: self.memories.get(step_idx),
            })
    }
}

/// An executed opcode of a [`FrameSteps`], with its snapshots.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FrameStep<'a> {
    /// Program counter before step execution
    pub pc: u32,
    /// Opcode to be executed
    pub op: OpCode,
    /// The stack before the step, if recorded.
    pub stack: Option<&'a [U256]>,
    /// The memory before the step, if recorded.
    pub memory: Option<&'a Bytes>,
}

/// A call trace arena with the steps of each node, indexed like the arena's
/// nodes.
#[derive(Clone, Debug)]
pub struct RecordedTraceArena {
    arena: CallTraceArena,
    /// One entry per node of `arena`; empty for nodes that executed no opcodes.
    steps: Vec<FrameSteps>,
}

impl RecordedTraceArena {
    /// Pairs `arena` with the steps recorded for its nodes, which may omit
    /// trailing nodes that executed no opcodes.
    pub fn new(arena: CallTraceArena, mut steps: Vec<FrameSteps>) -> Self {
        debug_assert!(
            steps.len() <= arena.nodes().len(),
            "steps recorded for nodes outside the arena"
        );
        steps.resize_with(arena.nodes().len(), FrameSteps::default);

        Self { arena, steps }
    }

    /// Returns the steps of the node at `node_idx`, or `None` if the arena has
    /// no such node.
    pub fn node_steps(&self, node_idx: usize) -> Option<&FrameSteps> {
        self.steps.get(node_idx)
    }

    /// Returns the steps of every node, indexed like the arena's nodes.
    pub fn all_node_steps(&self) -> &[FrameSteps] {
        &self.steps
    }
}

impl Default for RecordedTraceArena {
    fn default() -> Self {
        Self::new(CallTraceArena::default(), Vec::new())
    }
}

impl Deref for RecordedTraceArena {
    type Target = CallTraceArena;

    fn deref(&self) -> &Self::Target {
        &self.arena
    }
}
