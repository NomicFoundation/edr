//! Ported from `hardhat-network/stack-traces/source-maps.ts`.
#![allow(missing_docs)] // TODO: Document this module

use std::{cell::RefCell, collections::HashMap, rc::Rc};

use edr_evm::interpreter::OpCode;

use crate::build_model::{Instruction, JumpType, SourceFile, SourceLocation};

// See https://docs.soliditylang.org/en/latest/internals/source_mappings.html
pub struct SourceMapLocation {
    // Only -1 if the information is missing, the values are non-negative otherwise
    pub offset: i32,
    pub length: i32,
    pub file: i32,
}

pub struct SourceMap {
    pub location: SourceMapLocation,
    pub jump_type: JumpType,
}

fn jump_letter_to_jump_type(letter: &str) -> JumpType {
    match letter {
        "i" => JumpType::IntoFunction,
        "o" => JumpType::OutofFunction,
        _ => JumpType::NotJump,
    }
}

fn uncompress_sourcemaps(compressed: &str) -> Vec<SourceMap> {
    let mut mappings = Vec::new();

    let compressed_mappings = compressed.split(';');

    for (i, compressed_mapping) in compressed_mappings.enumerate() {
        let parts: Vec<&str> = compressed_mapping.split(':').collect();

        let has_parts0 = parts.first().map_or(false, |part| !part.is_empty());
        let has_parts1 = parts.get(1).map_or(false, |part| !part.is_empty());
        let has_parts2 = parts.get(2).map_or(false, |part| !part.is_empty());
        let has_parts3 = parts.get(3).map_or(false, |part| !part.is_empty());

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
                    parts[0].parse().unwrap_or_else(|_| {
                        panic!("Failed to parse offset at index {i}: `{}`", parts[0])
                    })
                } else {
                    mappings[i - 1].location.offset
                },
                length: if has_parts1 {
                    parts[1].parse().unwrap_or_else(|_| {
                        panic!("Failed to parse length at index {i}: `{}`", parts[1])
                    })
                } else {
                    mappings[i - 1].location.length
                },
                file: if has_parts2 {
                    parts[2].parse().unwrap_or_else(|_| {
                        panic!("Failed to parse file at index {i}: `{}`", parts[2])
                    })
                } else {
                    mappings[i - 1].location.file
                },
            },
            jump_type: if has_parts3 {
                jump_letter_to_jump_type(parts[3])
            } else {
                mappings[i - 1].jump_type
            },
        });
    }

    mappings
}

fn add_unmapped_instructions(instructions: &mut Vec<Instruction>, bytecode: &[u8]) {
    let last_instr_pc = instructions.last().map_or(0, |instr| instr.pc);

    let mut bytes_index = (last_instr_pc + 1) as usize;

    while bytecode.get(bytes_index) != Some(OpCode::INVALID.get()).as_ref() {
        let opcode = OpCode::new(bytecode[bytes_index]).expect("Invalid opcode");

        let push_data = if opcode.is_push() {
            let push_data = &bytecode[bytes_index..][..1 + opcode.info().immediate_size() as usize];

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
        };

        instructions.push(instruction);

        bytes_index += 1 + opcode.info().immediate_size() as usize;
    }
}

pub fn decode_instructions(
    bytecode: &[u8],
    compressed_sourcemaps: &str,
    file_id_to_source_file: &HashMap<u32, Rc<RefCell<SourceFile>>>,
    is_deployment: bool,
) -> Vec<Instruction> {
    let source_maps = uncompress_sourcemaps(compressed_sourcemaps);

    let mut instructions = Vec::new();

    let mut bytes_index = 0;

    while instructions.len() < source_maps.len() {
        let source_map = &source_maps[instructions.len()];

        let pc = bytes_index;
        let opcode = OpCode::new(bytecode[pc]).expect("Invalid opcode");

        let push_data = if opcode.is_push() {
            let push_data = &bytecode[bytes_index..][..1 + opcode.info().immediate_size() as usize];

            Some(push_data.to_vec())
        } else {
            None
        };

        let jump_type = match (opcode, source_map.jump_type) {
            (OpCode::JUMP | OpCode::JUMPI, JumpType::NotJump) => JumpType::InternalJump,
            _ => source_map.jump_type,
        };

        let location = if source_map.location.file == -1 {
            None
        } else {
            file_id_to_source_file
                .get(&(source_map.location.file as u32))
                .map(|file| {
                    Rc::new(SourceLocation::new(
                        file.clone(),
                        source_map.location.offset as u32,
                        source_map.location.length as u32,
                    ))
                })
        };

        let instruction = Instruction {
            pc: bytes_index as u32,
            opcode,
            jump_type,
            push_data,
            location,
        };

        instructions.push(instruction);

        bytes_index += 1 + opcode.info().immediate_size() as usize;
    }

    if is_deployment {
        add_unmapped_instructions(&mut instructions, bytecode);
    }

    instructions
}
