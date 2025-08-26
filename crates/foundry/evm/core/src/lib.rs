//! # foundry-evm-core
//!
//! Core EVM abstractions.

#![cfg_attr(not(test), warn(unused_crate_dependencies))]
#![allow(clippy::all, clippy::pedantic, clippy::restriction)]

#[macro_use]
extern crate tracing;

pub mod abi;

pub mod env;
pub use env::*;
pub mod backend;
pub mod constants;
pub mod contracts;
pub mod decode;
pub mod evm_context;
pub mod fork;
pub mod ic;
pub mod opts;
pub mod precompiles;
pub mod state_snapshot;
pub mod utils;
