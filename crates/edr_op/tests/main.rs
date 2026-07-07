//! Each Cargo integration test is a separate binary, so we use a single entry
//! point (`main.rs`) to minimise compile & link time.
//!
//! Inspired by [this blogpost](https://matklad.github.io/2021/02/27/delete-cargo-integration-tests.html).

// Integration test binaries are separate crate roots, so the crate's lib-level
// recursion_limit doesn't carry over. The deeply nested async futures exercised
// here exceed the default layout-computation recursion limit of 128.
#![recursion_limit = "256"]

mod integration;
