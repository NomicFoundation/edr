//! Defines EIP-712 types and a means of collecting EIP-712 canonical type
//! definitions from Solidity sources.

#![warn(missing_docs)]

pub mod collector;
pub mod parse;

pub use edr_solidity_parser_slang::ImportResolver;

/// An EIP-712 type definition in canonical form, paired with its
/// primary-type name.
///
/// Only [`Eip712Type::parse`] and the collector can construct one, and both
/// produce the canonical form.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Eip712Type {
    name: String,
    canonical_definition: String,
}

impl Eip712Type {
    /// Primary type name (the leftmost type in the canonical definition).
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Canonical EIP-712 type definition.
    pub fn canonical_definition(&self) -> &str {
        &self.canonical_definition
    }
}
