#![warn(missing_docs)]

//! The EDR EVM
//!
//! The EDR EVM exposes APIs for running and interacting with a multi-threaded
//! Ethereum Virtual Machine (or EVM).

pub use crate::{
    block::*,
    mempool::{MemPool, MemPoolAddTransactionError, OrderedTransaction},
    miner::*,
    random::RandomHashGenerator,
};

/// Types for Ethereum blocks.
pub mod block;
/// Types for managing Ethereum blockchain
pub mod blockchain;
pub(crate) mod collections;
/// Types for configuring the runtime.
pub mod config;
/// Types and constants for Ethereum improvements proposals (EIPs)
pub mod eips;
/// Types for interfacing with the evm.
pub mod evm;
/// Types for Ethereum hardforks
pub mod hardfork;
/// Types for EVM inspectors.
pub mod inspector;
/// Types for the EVM interpreter.
pub mod interpreter;
/// Types for the EVM journal.
pub mod journal;
/// Types for managing Ethereum mem pool
pub mod mempool;
mod miner;
/// Types for managing Ethereum precompiles
pub mod precompile;
pub(crate) mod random;
/// Types for Ethereum transaction receipts.
pub mod receipt;
/// Result types for EVM execution.
pub mod result;
/// Types and function for running the EVM.
pub mod runtime;
/// Types for chain specification.
pub mod spec;
/// Database types for managing Ethereum state
pub mod state;
/// Utilities for testing
#[cfg(any(test, feature = "test-utils"))]
pub mod test_utils;
/// Types used for tracing EVM calls
pub mod trace;
/// Types for Ethereum transactions
pub mod transaction;
