//! Utility types and functions used across the EDR codebase.

#![warn(missing_docs)]

use std::sync::Arc;

/// Types related to random number generation.
pub mod random;
/// Types related to the Rust type system.
pub mod types;

/// Trait for casting an `Arc<Self>` into an `Arc<T>`.
pub trait CastArc<T: ?Sized> {
    /// Converts an `Arc<Self>` into an `Arc<T>`.
    fn cast_arc(value: Arc<Self>) -> Arc<T>;
}
