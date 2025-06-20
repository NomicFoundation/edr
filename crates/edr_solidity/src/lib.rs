#![warn(missing_docs)]

//! Repository of information about contracts written in Solidity.

pub mod build_model;

pub mod contracts_identifier;

pub mod utils;

pub mod artifacts;
pub mod compiler;
pub mod contract_decoder;
pub mod exit_code;
pub mod library_utils;
pub mod linker;
pub mod nested_trace;
pub mod nested_tracer;
pub mod solidity_stack_trace;
pub mod solidity_tracer;

mod bytecode_trie;
mod error_inferrer;
mod mapped_inline_internal_functions_heuristics;
mod return_data;
mod source_map;
