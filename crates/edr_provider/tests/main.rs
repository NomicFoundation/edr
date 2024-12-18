//! Each Cargo integration test is a separate binary, so we use a single entry
//! point (`main.rs`) to minimise compile & link time.
//!
//! Inspired by [this blogpost](https://matklad.github.io/2021/02/27/delete-cargo-integration-tests.html).

/// Common types and functions for integration tests
pub mod common;
mod integration;
