//! Shared plumbing for parsing Solidity sources with Slang: import
//! resolution and compilation-unit building over on-disk files.

#![warn(missing_docs)]

mod compilation;
mod resolver;

pub use slang_solidity_v2::utils::LanguageVersion;

pub use crate::{
    compilation::{build_compilation_unit, language_version_for_solc},
    resolver::ImportResolver,
};
