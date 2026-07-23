//! Shared plumbing for parsing Solidity sources with Slang: import
//! resolution and compilation-unit building over on-disk files.

mod compilation;
mod resolver;

pub use crate::{
    compilation::{build_compilation_unit, supports_solc_version, UnsupportedSolcVersionError},
    resolver::ImportResolver,
};
