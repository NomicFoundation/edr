//! Utility types and functions used across the EDR codebase.

#![warn(missing_docs)]

use std::sync::Arc;

/// Types related to random number generation.
pub mod random;
/// Types related to the Rust type system.
pub mod types;

/// Trait for casting an `Arc<T>` into an `Arc<Self>`.
pub trait CastArcFrom<T: ?Sized> {
    /// Converts an `Arc<T>` into an `Arc<Self>`.
    fn cast_arc_from(value: Arc<T>) -> Arc<Self>;
}

/// Trait for casting an `Arc<Self>` into an `Arc<T>`.
pub trait CastArcInto<T: ?Sized> {
    /// Converts an `Arc<Self>` into an `Arc<T>`.
    fn cast_arc_into(self: Arc<Self>) -> Arc<T>;
}

impl<T: ?Sized, U: ?Sized + CastArcFrom<T>> CastArcInto<U> for T {
    fn cast_arc_into(self: Arc<Self>) -> Arc<U> {
        U::cast_arc_from(self)
    }
}
