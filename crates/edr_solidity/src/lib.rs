#![warn(missing_docs)]

//! Repository of information about contracts written in Solidity.

pub mod build_model;

pub mod contracts_identifier;

pub mod utils;

pub mod artifacts;
pub mod exit_code;
pub mod library_utils;
pub mod nested_trace;
pub mod nested_trace_decoder;
pub mod nested_tracer;
pub mod solidity_stack_trace;
pub mod solidity_tracer;

mod compiler;
mod error_inferrer;
mod mapped_inline_internal_functions_heuristics;
mod return_data;
mod source_map;
