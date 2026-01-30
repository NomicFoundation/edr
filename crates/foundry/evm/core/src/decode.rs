//! Various utilities to decode test results.

use alloy_primitives::Log;
use alloy_sol_types::SolEventInterface as _;

use crate::abi::console;

/// Decode a set of logs, only returning logs from `DSTest` logging events and
/// Hardhat's `console.log`
pub fn decode_console_logs(logs: &[Log]) -> Vec<String> {
    logs.iter().filter_map(decode_console_log).collect()
}

/// Decode a single log.
///
/// This function returns [None] if it is not a `DSTest` log or the result of a
/// Hardhat `console.log`.
pub fn decode_console_log(log: &Log) -> Option<String> {
    console::ds::ConsoleEvents::decode_log(log)
        .ok()
        .map(|decoded| decoded.to_string())
}
