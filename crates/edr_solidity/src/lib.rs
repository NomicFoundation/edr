#![warn(missing_docs)]

//! Repository of information about contracts written in Solidity.

pub mod build_model;

pub mod contracts_identifier;

pub mod utils;

pub mod artifacts;
pub mod library_utils;
pub mod message_trace;
pub mod vm_tracer;

pub mod compiler;
pub mod error_inferrer;
mod mapped_inline_internal_functions_heuristics;
pub mod return_data;
pub mod solidity_stack_trace;
pub mod solidity_tracer;
pub mod source_map;
pub mod vm_trace_decoder;
