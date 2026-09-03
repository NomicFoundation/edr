//! Shared plumbing for parsing Solidity sources with Slang: import
//! resolution and compilation-unit building over on-disk files.

#![warn(missing_docs)]

mod compilation;
mod resolver;

pub use crate::{
    compilation::{build_compilation_unit, UnsupportedSolcVersionError},
    resolver::ImportResolver,
};
