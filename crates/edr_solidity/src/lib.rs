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
pub mod source_map;
