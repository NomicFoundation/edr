#![warn(missing_docs)]

//! Ethereum types
//!
//! Ethereum types as needed by EDR. In particular, they are based on the same
//! primitive types as `revm`.

/// Ethereum block types
pub mod block;
/// Ethereum block spec
mod block_spec;
/// Types and constants for Ethereum improvements proposals (EIPs)
pub mod eips;
/// Ethereum fee history types
pub mod fee_history;
pub mod filter;
/// Ethereum result types
pub mod result;
/// Ethereum gas related types
pub mod reward_percentile;
#[cfg(feature = "serde")]
pub mod serde;

pub use c_kzg::{Blob, Bytes48};

pub use self::block_spec::{BlockSpec, BlockTag, Eip1898BlockSpec, PreEip1898BlockSpec};
