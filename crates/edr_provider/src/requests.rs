pub(crate) mod debug;
/// Ethereum RPC request types
pub(crate) mod eth;
/// Hardhat RPC request types
pub(crate) mod hardhat;
mod methods;
mod resolve;
mod serde;
/// Types and functions for validating JSON-RPC requests.
pub mod validation;

pub use crate::requests::{
    methods::{IntervalConfig, MethodInvocation},
    serde::{InvalidRequestReason, Timestamp},
};
