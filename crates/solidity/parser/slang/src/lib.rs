//! Shared plumbing for parsing Solidity sources with Slang: import
//! resolution and compilation-unit building over on-disk files.

#![warn(missing_docs)]

pub mod compilation;
pub mod exclusions;
pub mod resolver;

pub use slang_solidity_v2::utils::LanguageVersion;
