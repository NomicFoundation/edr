//! Utility functions for decoding the Solidity compiler source maps.

use edr_primitives::bytecode::opcode::OpCode;

use crate::build_model::{Instruction, JumpType};

/// Errors that can occur during source map decoding.
#[derive(Clone, Debug, thiserror::Error)]
pub enum SourceMapError {
    /// Failed to parse a numeric value in the source map.
    #[error("Failed to parse {field} at index {index}: `{value}`")]
    ParseError {
        field: String,
        index: usize,
        value: String,
    },
    /// Found an invalid opcode.
    #[error("Invalid opcode at index {index}: `{value}`")]
    InvalidOpcode { index: usize, value: String },
}

/// Source mapping used by the Solidity compiler as part of its AST output.
///
/// See <https://docs.soliditylang.org/en/latest/internals/source_mappings.html>.
pub struct SourceMapLocation {
    /// Byte-offset to the start of the range in the source file.
    // Only -1 if the information is missing, the values are non-negative otherwise
    pub offset: i32,
    /// Length of the source range in bytes.
    pub length: i32,
    /// Integer identifier of the source file.
    pub file: i32,
}

/// Source mapping for the bytecode. In addition to [`SourceMapLocation`], it
/// also contains the jump type.
pub struct SourceMap {
    /// Source mapping.
    pub location: SourceMapLocation,
    /// Jump type, i.e. into (`i`) or out of (`o`) function.
    pub jump_type: JumpType,
}

fn jump_letter_to_jump_type(letter: &str) -> JumpType {
    match letter {
        "i" => JumpType::IntoFunction,
        "o" => JumpType::OutofFunction,
        _ => JumpType::NotJump,
    }
}

pub(super) fn uncompress_sourcemaps(compressed: &str) -> Result<Vec<SourceMap>, SourceMapError> {
    let mut mappings = Vec::new();

    let compressed_mappings = compressed.split(';');

    for (i, compressed_mapping) in compressed_mappings.enumerate() {
        let parts: Vec<&str> = compressed_mapping.split(':').collect();

        let has_parts0 = parts.first().is_some_and(|part| !part.is_empty());
        let has_parts1 = parts.get(1).is_some_and(|part| !part.is_empty());
        let has_parts2 = parts.get(2).is_some_and(|part| !part.is_empty());
        let has_parts3 = parts.get(3).is_some_and(|part| !part.is_empty());

        let has_every_part = has_parts0 && has_parts1 && has_parts2 && has_parts3;

        // // See: https://github.com/nomiclabs/hardhat/issues/593
        if i == 0 && !has_every_part {
            mappings.push(SourceMap {
                jump_type: JumpType::NotJump,
                location: SourceMapLocation {
                    file: -1,
                    offset: 0,
                    length: 0,
                },
            });

            continue;
        }

        mappings.push(SourceMap {
            location: SourceMapLocation {
                offset: if has_parts0 {
                    parts
                        .first()
                        .expect("parts[0] should exist when has_parts0 is true")
                        .parse()
                        .map_err(|_err| SourceMapError::ParseError {
                            field: "offset".to_string(),
                            index: i,
                            value: (*parts
                                .first()
                                .expect("parts[0] should exist when has_parts0 is true"))
                            .to_string(),
                        })?
                } else {
                    mappings
                        .get(i - 1)
                        .expect("previous mapping should exist")
                        .location
                        .offset
                },
                length: if has_parts1 {
                    parts
                        .get(1)
                        .expect("parts[1] should exist when has_parts1 is true")
                        .parse()
                        .map_err(|_err| SourceMapError::ParseError {
                            field: "length".to_string(),
                            index: i,
                            value: (*parts
                                .get(1)
                                .expect("parts[1] should exist when has_parts1 is true"))
                            .to_string(),
                        })?
                } else {
                    mappings
                        .get(i - 1)
                        .expect("previous mapping should exist")
                        .location
                        .length
                },
                file: if has_parts2 {
                    parts
                        .get(2)
                        .expect("parts[2] should exist when has_parts2 is true")
                        .parse()
                        .map_err(|_err| SourceMapError::ParseError {
                            field: "file".to_string(),
                            index: i,
                            value: (*parts
                                .get(2)
                                .expect("parts[2] should exist when has_parts2 is true"))
                            .to_string(),
                        })?
                } else {
                    mappings
                        .get(i - 1)
                        .expect("previous mapping should exist")
                        .location
                        .file
                },
            },
            jump_type: if has_parts3 {
                jump_letter_to_jump_type(
                    parts
                        .get(3)
                        .expect("parts[3] should exist when has_parts3 is true"),
                )
            } else {
                mappings
                    .get(i - 1)
                    .expect("previous mapping should exist")
                    .jump_type
            },
        });
    }

    Ok(mappings)
}

pub(super) fn add_unmapped_instructions(
    instructions: &mut Vec<Instruction>,
    bytecode: &[u8],
) -> Result<(), SourceMapError> {
    let mut bytes_index = instructions.last().map_or(0, |instr| {
        // On the odd chance that the last instruction is a PUSH, we make sure
        // to include any immediate data that might be present.
        instr.pc as usize + 1 + instr.opcode.info().immediate_size() as usize
    });

    while bytecode.get(bytes_index) != Some(OpCode::INVALID.get()).as_ref() {
        let opcode = OpCode::new(
            *bytecode
                .get(bytes_index)
                .expect("bytes_index should be within bytecode bounds"),
        )
        .ok_or_else(|| SourceMapError::InvalidOpcode {
            index: bytes_index,
            value: format!(
                "{:02x}",
                *bytecode
                    .get(bytes_index)
                    .expect("bytes_index should be within bytecode bounds")
            ),
        })?;

        let push_data = if opcode.is_push() {
            let push_data = bytecode
                .get(bytes_index..)
                .expect("bytes_index should be within bytecode bounds")
                .get(..1 + opcode.info().immediate_size() as usize)
                .expect("bytecode should have enough bytes for push data");

            Some(push_data.to_vec())
        } else {
            None
        };

        let jump_type = if matches!(opcode, OpCode::JUMP | OpCode::JUMPI) {
            JumpType::InternalJump
        } else {
            JumpType::NotJump
        };

        let instruction = Instruction {
            pc: bytes_index as u32,
            opcode,
            jump_type,
            push_data,
            location: None,
            inline_call_sites: Box::default(),
        };

        instructions.push(instruction);

        bytes_index += 1 + opcode.info().immediate_size() as usize;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use edr_primitives::bytecode::opcode;

    use super::*;

    #[test]
    fn unmapped_instruction_opcode_boundary() {
        let bytecode = &[opcode::PUSH2, 0xde, 0xad, opcode::STOP, opcode::INVALID];

        let mut instructions = vec![Instruction {
            pc: 0,
            opcode: OpCode::PUSH2,
            jump_type: JumpType::NotJump,
            push_data: Some(vec![0xde, 0xad]),
            location: None,
            inline_call_sites: Box::default(),
        }];

        // Make sure we start decoding from opcode::STOP rather than from inside
        // the push data.
        add_unmapped_instructions(&mut instructions, bytecode).unwrap();

        assert!(matches!(
            instructions.last(),
            Some(Instruction {
                opcode: OpCode::STOP,
                ..
            })
        ));
    }
}
