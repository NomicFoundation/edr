//! Tiny EVM assembler for hand-built test contracts.

pub use edr_primitives::bytecode::opcode;
use edr_primitives::Bytes;

/// Assembles a contract's runtime code opcode by opcode, encapsulating the
/// deploy-wrapper and length bookkeeping.
#[derive(Default)]
pub struct BytecodeBuilder {
    runtime: Vec<u8>,
}

impl BytecodeBuilder {
    /// Appends a plain opcode.
    pub fn opcode(&mut self, opcode: u8) -> &mut Self {
        self.runtime.push(opcode);
        self
    }

    /// Appends an opcode followed by its one-byte immediate operand.
    pub fn opcode_with_immediate(&mut self, opcode: u8, immediate: u8) -> &mut Self {
        self.runtime.extend([opcode, immediate]);
        self
    }

    /// Appends `PUSH1 value`.
    pub fn push1(&mut self, value: u8) -> &mut Self {
        self.opcode_with_immediate(opcode::PUSH1, value)
    }

    /// Appends the `PUSH` of `bytes`, which must be 1 to 32 bytes long.
    pub fn push(&mut self, bytes: &[u8]) -> &mut Self {
        let len = u8::try_from(bytes.len()).expect("push fits in 32 bytes");
        assert!((1..=32).contains(&len), "push takes 1 to 32 bytes");

        self.runtime.push(opcode::PUSH0 + len);
        self.runtime.extend_from_slice(bytes);
        self
    }

    /// Appends code returning the top of the stack as a 32-byte word.
    pub fn return_top_word(&mut self) -> &mut Self {
        self.push1(0)
            .opcode(opcode::MSTORE)
            .push1(0x20)
            .push1(0)
            .opcode(opcode::RETURN)
    }

    /// The assembled runtime code, for contracts seeded directly into state
    /// rather than deployed.
    pub fn runtime(self) -> Bytes {
        self.runtime.clone().into()
    }

    /// Init bytecode wrapping the runtime in the standard constructor that
    /// copies it out as the deployed code.
    pub fn deployable(self) -> Bytes {
        const CONSTRUCTOR_LEN: u8 = 12;

        let runtime_len = u8::try_from(self.runtime.len()).expect("runtime length fits a PUSH1");
        let mut code = vec![
            opcode::PUSH1,
            runtime_len,
            opcode::PUSH1,
            CONSTRUCTOR_LEN,
            opcode::PUSH1,
            0x00,
            opcode::CODECOPY,
            opcode::PUSH1,
            runtime_len,
            opcode::PUSH1,
            0x00,
            opcode::RETURN,
        ];
        assert_eq!(code.len(), usize::from(CONSTRUCTOR_LEN));

        code.extend_from_slice(&self.runtime);
        code.into()
    }
}
