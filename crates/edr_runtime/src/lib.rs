#![warn(missing_docs)]

//! The EDR EVM
//!
//! The EDR EVM exposes APIs for running and interacting with a multi-threaded
//! Ethereum Virtual Machine (or EVM).

/// Types for EVM inspectors.
pub mod inspector;
pub mod overrides;
/// Types for Ethereum transactions
pub mod transaction;
