//! Defines EIP-712 types and a means of collecting EIP-712 canonical type
//! definitions from Solidity sources.

mod collector;
pub mod parse;
mod provider;
mod resolver;

pub use crate::{
    collector::CollectError,
    provider::{CachedEip712Provider, Eip712Root, SharedEip712Provider},
    resolver::ImportResolver,
};

/// An EIP-712 type definition in canonical form, paired with its
/// primary-type name.
///
/// Only [`Eip712TypeDef::parse`] can construct one, which guarantees the
/// canonical-form invariant.
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

    /// Canonical EIP-712 type definition, as produced by
    /// [`EncodeType::canonicalize`].
    pub fn canonical_definition(&self) -> &str {
        &self.canonical_definition
    }
}
