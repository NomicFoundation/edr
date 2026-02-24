#![warn(missing_docs)]

//! Repository of information about contracts written in Solidity.

pub mod artifacts;
pub mod build_model;
pub mod compiler;
pub mod config;
pub mod contract_decoder;
pub mod contracts_identifier;
pub mod exit_code;
pub mod library_utils;
pub mod linker;
pub mod nested_trace;
pub mod nested_tracer;
pub mod proxy_detection;
pub mod solidity_stack_trace;
pub mod solidity_tracer;
pub mod tracing;
pub mod utils;

mod bytecode_trie;
mod error_inferrer;
mod mapped_inline_internal_functions_heuristics;
pub mod return_data;
mod source_map;
