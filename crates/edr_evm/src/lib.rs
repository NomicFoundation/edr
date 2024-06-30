#![warn(missing_docs)]

//! The EDR EVM
//!
//! The EDR EVM exposes APIs for running and interacting with a multi-threaded
//! Ethereum Virtual Machine (or EVM).

pub use crate::{
    block::*,
    debug::{DebugContext, GetContextData},
    debug_trace::{
        debug_trace_transaction, execution_result_to_debug_result,
        register_eip_3155_and_raw_tracers_handles, register_eip_3155_tracer_handles,
        DebugTraceConfig, DebugTraceError, DebugTraceLogItem, DebugTraceResult,
        DebugTraceResultWithTraces, Eip3155AndRawTracers, TracerEip3155,
    },
    mempool::{MemPool, MemPoolAddTransactionError, OrderedTransaction},
    miner::*,
    random::RandomHashGenerator,
    runtime::{dry_run, guaranteed_dry_run, run, SyncDatabase},
};

/// Types for managing Ethereum blockchain
pub mod blockchain;

/// Database types for managing Ethereum state
pub mod state;

/// Types used for tracing EVM calls
pub mod trace;

mod block;
/// Types for chain specification.
pub mod chain_spec;
pub(crate) mod collections;
mod debug;
mod debug_trace;
/// Types for Ethereum hardforks
pub mod hardfork;
/// Types for managing Ethereum mem pool
pub mod mempool;
mod miner;
pub(crate) mod random;
mod runtime;
/// Utilities for testing
#[cfg(any(test, feature = "test-utils"))]
pub mod test_utils;
/// Types for Ethereum transactions
pub mod transaction;

/// Types for interfacing with the evm
pub mod evm {
    pub use revm::{handler, FrameOrResult, FrameResult};
}

/// Types for interfacing with the interpreter
pub mod interpreter {
    pub use revm::interpreter::*;
}

/// Types for managing Ethereum precompiles
pub mod precompile {
    pub use revm::precompile::{u64_to_address, PrecompileSpecId, Precompiles};
}
