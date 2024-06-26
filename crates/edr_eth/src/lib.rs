#![warn(missing_docs)]

//! Ethereum types
//!
//! Ethereum types as needed by EDR. In particular, they are based on the same
//! primitive types as `revm`.

/// Ethereum account types
pub mod account;
/// Parent beacon types and constants
pub mod beacon;
/// Ethereum block types
pub mod block;
/// Ethereum block spec
mod block_spec;
/// Ethereum fee history types
pub mod fee_history;
/// Ethereum types for filter-based RPC methods
pub mod filter;
/// Ethereum log types
pub mod log;
/// Ethereum receipt types
pub mod receipt;
/// Ethereum gas related types
pub mod reward_percentile;
/// RLP traits and functions
pub mod rlp;
#[cfg(feature = "serde")]
pub mod serde;
/// Ethereum signature types
pub mod signature;
/// Specification of hardforks
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

pub use c_kzg::{Blob, Bytes48, BYTES_PER_BLOB, BYTES_PER_COMMITMENT, BYTES_PER_PROOF};
pub use revm_primitives::{
    address,
    alloy_primitives::{Bloom, BloomInput, ChainId, B512, B64, U128, U160, U64, U8},
    db, env, hex, hex_literal, result, AccessList, AccessListItem, AccountInfo, Address, Bytecode,
    Bytes, HashMap, HashSet, SpecId, B256, KECCAK_EMPTY, MAX_INITCODE_SIZE, U256,
};

pub use self::block_spec::{BlockSpec, BlockTag, Eip1898BlockSpec, PreEip1898BlockSpec};

/// A secret key
pub type Secret = B256;
/// A public key
pub type Public = B512;
