//! # foundry-evm
//!
//! Main Foundry EVM backend abstractions.

#![warn(unreachable_pub, unused_crate_dependencies, rust_2018_idioms)]

#[macro_use]
extern crate tracing;

// These crates are used by foundry-evm
use hex as _;
use serde_json as _;
use tokio as _;

pub mod executors;
pub mod inspectors;

pub use foundry_evm_core::{
    abi, backend, constants, contracts, decode, evm_context, fork, opts, utils,
};
pub use foundry_evm_coverage as coverage;
pub use foundry_evm_fuzz as fuzz;
pub use foundry_evm_traces as traces;
// TODO: We should probably remove these, but it's a pretty big breaking change.
#[doc(hidden)]
pub use revm;

#[doc(hidden)]
#[deprecated = "use `{hash_map, hash_set, HashMap, HashSet}` in `std::collections` or `revm::primitives` instead"]
pub mod hashbrown {
    pub use revm::primitives::{hash_map, hash_set, HashMap, HashSet};
}
