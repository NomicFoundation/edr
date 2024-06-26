//! Ported from `hardhat-network/stack-traces/source-maps.ts`.

use std::{collections::HashMap, rc::Rc};

use napi::{bindgen_prelude::Buffer, Env};
use napi_derive::napi;

use super::model::{SourceFile, SourceLocation};
use crate::{
    trace::{
        model::{Instruction, JumpType},
        opcodes::{get_opcode_length, get_push_length, is_jump, is_push, Opcode},
    },
    utils::ClassInstanceRef,
};

// See https://docs.soliditylang.org/en/latest/internals/source_mappings.html
#[napi(object)]
pub struct SourceMapLocation {
    // Only -1 if the information is missing, the values are non-negative otherwise
    pub offset: i32,
    pub length: i32,
    pub file: i32,
}

#[napi(object)]
pub struct SourceMap {
    pub location: SourceMapLocation,
    pub jump_type: JumpType,
}

fn jump_letter_to_jump_type(letter: &str) -> JumpType {
    match letter {
        "i" => JumpType::INTO_FUNCTION,
        "o" => JumpType::OUTOF_FUNCTION,
        _ => JumpType::NOT_JUMP,
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
                jump_type: JumpType::NOT_JUMP,
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

fn add_unmapped_instructions(
    instructions: &mut Vec<Instruction>,
    bytecode: &[u8],
) -> napi::Result<()> {
    let last_instr_pc = instructions.last().map_or(0, |instr| instr.pc);

    let mut bytes_index = (last_instr_pc + 1) as usize;

    while bytecode.get(bytes_index) != Some(Opcode::INVALID as u8).as_ref() {
        let opcode = Opcode::from_repr(bytecode[bytes_index]).expect("Invalid opcode");

        let push_data: Option<Buffer> = if is_push(opcode) {
            let push_data = &bytecode
                [(bytes_index + 1)..(bytes_index + 1 + (get_push_length(opcode) as usize))];

            Some(Buffer::from(push_data))
        } else {
            None
        };

        let jump_type = if is_jump(opcode) {
            JumpType::INTERNAL_JUMP
        } else {
            JumpType::NOT_JUMP
        };

        let instruction = Instruction::new(bytes_index as u32, opcode, jump_type, push_data, None)?;

        instructions.push(instruction);

        bytes_index += get_opcode_length(opcode) as usize;
    }

    Ok(())
}

pub fn decode_instructions(
    bytecode: &[u8],
    compressed_sourcemaps: &str,
    file_id_to_source_file: &HashMap<u32, Rc<ClassInstanceRef<SourceFile>>>,
    is_deployment: bool,
    env: Env,
) -> napi::Result<Vec<Instruction>> {
    let source_maps = uncompress_sourcemaps(compressed_sourcemaps);

    let mut instructions = Vec::new();

    let mut bytes_index = 0;

    while instructions.len() < source_maps.len() {
        let source_map = &source_maps[instructions.len()];

        let pc = bytes_index;
        let opcode = Opcode::from_repr(bytecode[pc]).expect("Invalid opcode");

        let push_data = if is_push(opcode) {
            let length = get_push_length(opcode);
            let push_data = &bytecode[(bytes_index + 1)..(bytes_index + 1 + (length as usize))];

            Some(Buffer::from(push_data))
        } else {
            None
        };

        let jump_type = if is_jump(opcode) && source_map.jump_type == JumpType::NOT_JUMP {
            JumpType::INTERNAL_JUMP
        } else {
            source_map.jump_type
        };

        let location = if source_map.location.file == -1 {
            None
        } else {
            match file_id_to_source_file.get(&(source_map.location.file as u32)) {
                Some(file) => {
                    let location = SourceLocation::new(
                        file.clone(),
                        source_map.location.offset as u32,
                        source_map.location.length as u32,
                    )
                    .into_instance(env)?;

                    Some(ClassInstanceRef::from_obj(location, env)?)
                }
                None => None,
            }
        };

        let instruction =
            Instruction::new(bytes_index as u32, opcode, jump_type, push_data, location)?;

        instructions.push(instruction);

        bytes_index += get_opcode_length(opcode) as usize;
    }

    if is_deployment {
        add_unmapped_instructions(&mut instructions, bytecode)?;
    }

    Ok(instructions)
}
