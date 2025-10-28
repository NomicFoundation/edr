#![warn(missing_docs)]

//! The EDR EVM
//!
//! The EDR EVM exposes APIs for running and interacting with a multi-threaded
//! Ethereum Virtual Machine (or EVM).

pub use crate::{
    mempool::{MemPool, MemPoolAddTransactionError, OrderedTransaction},
    miner::*,
};

/// Types for EVM inspectors.
pub mod inspector;
/// Types for the EVM journal.
pub mod journal;
/// Types for managing Ethereum mem pool
pub mod mempool;
mod miner;
pub mod overrides;
/// Utilities for testing
#[cfg(any(test, feature = "test-utils"))]
pub mod test_utils;
/// Types used for tracing EVM calls
pub mod trace;
/// Types for Ethereum transactions
pub mod transaction;
