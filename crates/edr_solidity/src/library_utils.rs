//! Utility functions for working with libraries in Solidity.
//! Ported from `hardhat-network/stack-traces/library-utils.ts`.

use anyhow::Context;
use edr_eth::hex;

use crate::artifacts::CompilerOutputBytecode;

/// Normalizes the compiler output bytecode by replacing the library addresses
/// with zeros.
pub fn normalize_compiler_output_bytecode(
    mut compiler_output_bytecode_object: String,
    addresses_positions: &[u32],
) -> Result<Vec<u8>, anyhow::Error> {
    const ZERO_ADDRESS: &str = "0000000000000000000000000000000000000000";

    for &pos in addresses_positions {
        compiler_output_bytecode_object =
            link_hex_string_bytecode(compiler_output_bytecode_object, ZERO_ADDRESS, pos)?;
    }

    Ok(hex::decode(compiler_output_bytecode_object)?)
}

/// Retrieves the positions of the library addresses in the bytecode.
pub fn get_library_address_positions(bytecode_output: &CompilerOutputBytecode) -> Vec<u32> {
    bytecode_output
        .link_references
        .values()
        .flat_map(|libs| {
            libs.values()
                .flat_map(|references| references.iter().map(|reference| reference.start))
        })
        .collect()
}

/// For the hex string, replaces the bytecode at the given position with the
/// given address. # Panics
/// This function panics if replacing the address would result in an invalid
/// UTF-8 string.
pub fn link_hex_string_bytecode(
    code: String,
    address: &str,
    position: u32,
) -> Result<String, anyhow::Error> {
    let address = address.strip_prefix("0x").unwrap_or(address);
    let pos = position as usize;

    let mut bytes = code.into_bytes();
    bytes
        .get_mut(pos * 2..pos * 2 + address.len())
        .expect("position and address length should be within bytecode bounds")
        .copy_from_slice(address.as_bytes());
    String::from_utf8(bytes).with_context(|| {
        format!("Invalid UTF-8 in hex strings for code or address. The address is '{address}'")
    })
}
