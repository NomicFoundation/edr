//! Port of the hardhat-network's `library-utils.ts` to Rust.

use edr_eth::Address;
use edr_evm::hex;
use edr_solidity::{artifacts::CompilerOutputBytecode, library_utils};
use napi::bindgen_prelude::{Buffer, Uint8Array};
use napi_derive::napi;

use super::{model::ImmutableReference, opcodes::Opcode};

#[napi]
pub fn get_library_address_positions(bytecode_output: serde_json::Value) -> Vec<u32> {
    let bytecode_output: CompilerOutputBytecode = serde_json::from_value(bytecode_output).unwrap();

    library_utils::get_library_address_positions(&bytecode_output)
}

/// Normalizes the compiler output bytecode by replacing the library addresses
/// with zeros.
pub fn normalize_compiler_output_bytecode(
    mut compiler_output_bytecode_object: String,
    addresses_positions: &[u32],
) -> napi::Result<Buffer> {
    const ZERO_ADDRESS: &str = "0000000000000000000000000000000000000000";

    for &pos in addresses_positions {
        compiler_output_bytecode_object = edr_solidity::library_utils::link_hex_string_bytecode(
            compiler_output_bytecode_object,
            ZERO_ADDRESS,
            pos,
        );
    }

    Ok(Buffer::from(
        hex::decode(compiler_output_bytecode_object)
            .map_err(|e| napi::Error::from_reason(format!("Failed to decode hex: {e:?}")))?,
    ))
}

#[napi]
pub fn link_hex_string_bytecode(code: String, address: String, position: u32) -> String {
    edr_solidity::library_utils::link_hex_string_bytecode(code, &address, position)
}

#[napi]
pub fn zero_out_addresses(mut code: Uint8Array, addresses_positions: Vec<u32>) {
    for pos in addresses_positions {
        code[pos as usize..][..Address::len_bytes()].fill(0);
    }
}

#[napi]
pub fn zero_out_slices(mut code: Uint8Array, pos: Vec<ImmutableReference>) {
    for ImmutableReference { start, length } in &pos {
        code[*start as usize..][..*length as usize].fill(0);
    }
}

#[napi]
pub fn normalize_library_runtime_bytecode_if_necessary(code: Uint8Array) {
    // Libraries' protection normalization:
    // Solidity 0.4.20 introduced a protection to prevent libraries from being
    // called directly. This is done by modifying the code on deployment, and
    // hard-coding the contract address. The first instruction is a PUSH20 of
    // the address, which we zero-out as a way of normalizing it. Note that it's
    // also zeroed-out in the compiler output.
    if code.first().copied() == Some(Opcode::PUSH20 as u8) {
        zero_out_addresses(code, vec![1]);
    }
}
