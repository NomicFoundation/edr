//! Compact per-frame opcode steps, recorded next to a [`CallTraceArena`]
//! whose own `CallTraceStep`s are disabled.

use std::ops::{Deref, DerefMut};

use edr_primitives::{bytecode::opcode::OpCode, Bytes, U256};
use revm_inspectors::tracing::CallTraceArena;

/// An executed opcode.
///
/// Unlike `revm_inspectors`' `CallTraceStep`, it holds only what EDR reads from
/// steps: the PC and opcode, used by stack traces and `traces()`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RecordedStep {
    /// Program counter before step execution
    pub pc: u32,
    /// Opcode to be executed
    pub op: OpCode,
}

/// The steps of a call trace node.
#[derive(Clone, Debug, Default)]
pub struct NodeSteps {
    /// The executed opcodes.
    pub steps: Vec<RecordedStep>,
    /// The stack before each step. Empty unless stack snapshots are recorded.
    pub stacks: Vec<Box<[U256]>>,
    /// The memory before each step. Empty unless memory snapshots are
    /// recorded.
    pub memories: Vec<Bytes>,
}

impl NodeSteps {
    /// Returns the stack before the step at `step_idx`, if recorded.
    pub fn stack(&self, step_idx: usize) -> Option<&[U256]> {
        self.stacks.get(step_idx).map(AsRef::as_ref)
    }

    /// Returns the memory before the step at `step_idx`, if recorded.
    pub fn memory(&self, step_idx: usize) -> Option<&Bytes> {
        self.memories.get(step_idx)
    }
}

/// A call trace arena with the steps of each node, indexed like the arena's
/// nodes.
#[derive(Clone, Debug, Default)]
pub struct RecordedTraceArena {
    /// The call trace arena, without steps.
    pub arena: CallTraceArena,
    /// The steps of each node.
    pub steps: Vec<NodeSteps>,
}

impl RecordedTraceArena {
    /// Returns the steps of the node at `node_idx`.
    pub fn node_steps(&self, node_idx: usize) -> &[RecordedStep] {
        self.steps
            .get(node_idx)
            .map_or(&[], |node| node.steps.as_slice())
    }

    /// Returns the steps and snapshots of the node at `node_idx`.
    pub fn node(&self, node_idx: usize) -> Option<&NodeSteps> {
        self.steps.get(node_idx)
    }
}

impl Deref for RecordedTraceArena {
    type Target = CallTraceArena;

    fn deref(&self) -> &Self::Target {
        &self.arena
    }
}

impl DerefMut for RecordedTraceArena {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.arena
    }
}
