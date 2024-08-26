//! cli arguments for configuring the evm settings
use alloy_primitives::Address;
use rustc_hash::FxHashMap;

/// Map keyed by breakpoints char to their location (contract address, pc)
pub type Breakpoints = FxHashMap<char, (Address, usize)>;
