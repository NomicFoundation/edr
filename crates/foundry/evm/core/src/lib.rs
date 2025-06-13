//! # foundry-evm-core
//!
//! Core EVM abstractions.

#![cfg_attr(not(test), warn(unused_crate_dependencies))]

#[macro_use]
extern crate tracing;

mod ic;

pub mod abi;
pub mod backend;
pub mod constants;
pub mod contracts;
pub mod decode;
pub mod evm_context;
pub mod fork;
pub mod opcodes;
pub mod opts;
pub mod precompiles;
pub mod state_snapshot;
pub mod utils;
