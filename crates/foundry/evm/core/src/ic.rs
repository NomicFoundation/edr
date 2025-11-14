use alloy_primitives::map::rustc_hash::FxHashMap;
use eyre::Result;
use revm::bytecode::opcode::{OpCode, PUSH0, PUSH1, PUSH32};
use serde::Serialize;

/// Maps from program counter to instruction counter.
///
/// Inverse of [`IcPcMap`].
#[derive(Debug, Clone, Serialize)]
#[serde(transparent)]
pub struct PcIcMap {
    pub inner: FxHashMap<u32, u32>,
}

impl PcIcMap {
    /// Creates a new `PcIcMap` for the given code.
    pub fn new(code: &[u8]) -> Self {
        Self {
            inner: make_map::<true>(code),
        }
    }

    /// Returns the length of the map.
    pub fn len(&self) -> usize {
        self.inner.len()
    }

    /// Returns `true` if the map is empty.
    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }

    /// Returns the instruction counter for the given program counter.
    pub fn get(&self, pc: u32) -> Option<u32> {
        self.inner.get(&pc).copied()
    }
}

/// Map from instruction counter to program counter.
///
/// Inverse of [`PcIcMap`].
pub struct IcPcMap {
    pub inner: FxHashMap<u32, u32>,
}

impl IcPcMap {
    /// Creates a new `IcPcMap` for the given code.
    pub fn new(code: &[u8]) -> Self {
        Self {
            inner: make_map::<false>(code),
        }
    }

    /// Returns the length of the map.
    pub fn len(&self) -> usize {
        self.inner.len()
    }

    /// Returns `true` if the map is empty.
    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }

    /// Returns the program counter for the given instruction counter.
    pub fn get(&self, ic: u32) -> Option<u32> {
        self.inner.get(&ic).copied()
    }
}

fn make_map<const PC_FIRST: bool>(code: &[u8]) -> FxHashMap<u32, u32> {
    assert!(u32::try_from(code.len()).is_ok(), "bytecode is too big");

    let mut map = FxHashMap::with_capacity_and_hasher(
        code.len(),
        revm_primitives::map::rustc_hash::FxBuildHasher,
    );

    let mut pc = 0usize;
    let mut cumulative_push_size = 0usize;
    while pc < code.len() {
        let ic = pc - cumulative_push_size;
        if PC_FIRST {
            map.insert(pc as u32, ic as u32);
        } else {
            map.insert(ic as u32, pc as u32);
        }

        let opcode = code.get(pc).expect("pc should be within code bounds");
        if (PUSH1..=PUSH32).contains(opcode) {
            // Skip the push bytes.
            let push_size = (opcode - PUSH0) as usize;
            pc += push_size;
            cumulative_push_size += push_size;
        }

        pc += 1;
    }

    map.shrink_to_fit();

    map
}

/// Represents a single instruction consisting of the opcode and its immediate
/// data.
pub struct Instruction {
    /// `OpCode`, if it could be decoded.
    pub op: Option<OpCode>,
    /// Immediate data following the opcode.
    pub immediate: Box<[u8]>,
    /// Program counter of the opcode.
    pub pc: u32,
}

/// Decodes raw opcode bytes into [`Instruction`]s.
pub fn decode_instructions(code: &[u8]) -> Result<Vec<Instruction>> {
    assert!(u32::try_from(code.len()).is_ok(), "bytecode is too big");

    let mut pc = 0usize;
    let mut steps = Vec::new();

    while pc < code.len() {
        let op = code.get(pc).copied().and_then(OpCode::new);
        let next_pc = pc + 1;
        let immediate_size = op.map_or(0, |op| op.info().immediate_size()) as usize;
        let is_normal_push = op.is_some_and(revm::bytecode::OpCode::is_push);

        if !is_normal_push && next_pc + immediate_size > code.len() {
            eyre::bail!("incomplete sequence of bytecode");
        }

        // Ensure immediate is padded if needed.
        let immediate_end = (next_pc + immediate_size).min(code.len());
        let mut immediate = vec![0u8; immediate_size];
        let immediate_part = code
            .get(next_pc..immediate_end)
            .expect("immediate_end should be within code bounds");
        let dest = immediate
            .get_mut(..immediate_part.len())
            .expect("immediate buffer should accommodate immediate_part");
        dest.copy_from_slice(immediate_part);

        steps.push(Instruction {
            op,
            pc: pc as u32,
            immediate: immediate.into_boxed_slice(),
        });

        pc = next_pc + immediate_size;
    }

    Ok(steps)
}

#[cfg(test)]
pub mod tests {
    use super::*;

    #[test]
    fn decode_push2_and_stop() -> Result<()> {
        // 0x61 0xAA 0xBB = PUSH2 0xAABB
        // 0x00           = STOP
        let code = vec![0x61, 0xAA, 0xBB, 0x00];
        let insns = decode_instructions(&code)?;

        // PUSH2 then STOP
        assert_eq!(insns.len(), 2);

        // PUSH2 at pc = 0
        let i0 = &insns[0];
        assert_eq!(i0.pc, 0);
        assert_eq!(i0.op, Some(OpCode::PUSH2));
        assert_eq!(i0.immediate.as_ref(), &[0xAA, 0xBB]);

        // STOP at pc = 3
        let i1 = &insns[1];
        assert_eq!(i1.pc, 3);
        assert_eq!(i1.op, Some(OpCode::STOP));
        assert!(i1.immediate.is_empty());

        Ok(())
    }

    #[test]
    fn decode_arithmetic_ops() -> Result<()> {
        // 0x01 = ADD, 0x02 = MUL, 0x03 = SUB, 0x04 = DIV
        let code = vec![0x01, 0x02, 0x03, 0x04];
        let insns = decode_instructions(&code)?;

        assert_eq!(insns.len(), 4);

        let expected = [
            (0, OpCode::ADD),
            (1, OpCode::MUL),
            (2, OpCode::SUB),
            (3, OpCode::DIV),
        ];
        for ((pc, want_op), insn) in expected.iter().zip(insns.iter()) {
            assert_eq!(insn.pc, *pc);
            assert_eq!(insn.op, Some(*want_op));
            assert!(insn.immediate.is_empty());
        }

        Ok(())
    }
}
