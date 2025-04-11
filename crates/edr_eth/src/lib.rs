#![warn(missing_docs)]

//! Ethereum types
//!
//! Ethereum types as needed by EDR. In particular, they are based on the same
//! primitive types as `revm`.

/// Ethereum account types
pub mod account;
/// Ethereum block types
pub mod block;
/// Ethereum block spec
mod block_spec;
/// Types and constants for Ethereum improvements proposals (EIPs)
pub mod eips;
/// Ethereum fee history types
pub mod fee_history;
/// Ethereum types for filter-based RPC methods
pub mod filter;
/// L1 chain specification.
pub mod l1;
/// Ethereum log types
pub mod log;
/// Ethereum receipt types
pub mod receipt;
/// Ethereum result types
pub mod result;
/// Ethereum gas related types
pub mod reward_percentile;
/// RLP traits and functions
pub mod rlp;
#[cfg(feature = "serde")]
pub mod serde;
/// Ethereum signature types
pub mod signature;
/// Ethereum chain spec traits
pub mod spec;
/// Ethereum state types and functions
pub mod state;
/// Ethereum transaction types
pub mod transaction;
/// Ethereum trie functions
pub mod trie;
/// Ethereum utility functions
pub mod utils;
pub mod withdrawal;

pub use c_kzg::{Blob, Bytes48};
pub use revm_bytecode::{self as bytecode, Bytecode};
pub use revm_primitives::{
    Address, B256, Bytes, HashMap, HashSet, KECCAK_EMPTY, MAX_INITCODE_SIZE, U256, address,
    alloy_primitives::{B64, B512, Bloom, BloomInput, ChainId, U8, U64, U128, U160},
    b256, bytes, hash_map, hash_set, hex, hex_literal, keccak256,
};

pub use self::block_spec::{BlockSpec, BlockTag, Eip1898BlockSpec, PreEip1898BlockSpec};

/// A secret key
pub type Secret = B256;
/// A public key
pub type Public = B512;
